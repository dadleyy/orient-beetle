use serde::{Deserialize, Serialize};

#[cfg(debug_assertions)]
const COOKIE_SET_FLAGS: &'static str = "Max-Age=86400; Path=/; SameSite=Strict; HttpOnly";

#[cfg(not(debug_assertions))]
const COOKIE_SET_FLAGS: &'static str = "Max-Age=86400; Path=/; SameSite=Strict; HttpOnly; Secure";

#[derive(Debug, Deserialize)]
struct AuthCodeResponse {
  access_token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct UserInfo {
  sub: String,
  nickname: String,
  email: String,
  picture: String,
}

#[derive(Debug, Serialize)]
struct AuthCodeRequest {
  grant_type: String,
  client_id: String,
  client_secret: String,
  redirect_uri: String,
  code: String,
}

/// With a token provided by our oauth provider, this will return to us all of the user
/// information that is available to us.
async fn fetch_user<U, T>(uri: U, token: T) -> Option<UserInfo>
where
  T: std::fmt::Display,
  U: AsRef<str>,
{
  let mut res = surf::get(uri.as_ref())
    .header("Authorization", format!("Bearer {}", token))
    .await
    .ok()?;

  if res.status() != surf::StatusCode::Ok {
    log::warn!("bad response status - '{:?}'", res.status());
    return None;
  }

  log::debug!("loaded info with status '{}', attempting to parse", res.status());
  res.body_json::<UserInfo>().await.ok()
}

async fn token_from_response(response: &mut surf::Response) -> Option<String> {
  let status = response.status();

  match status {
    surf::StatusCode::Ok => log::debug!("good response from auth provider token api"),
    other => {
      log::warn!("bad status code from token response - '{:?}'", other);
      return None;
    }
  };

  response
    .body_json::<AuthCodeResponse>()
    .await
    .ok()
    .map(|body| body.access_token)
}

/// Route: complete
///
/// Once the user has logged in via the oauth provider, they are sent back to this route
/// with a "code" present in the url. That code can be exchanged for a token and we will
/// either create a new user entry or update an existing one. Afterwards, we will redirect
/// them to the UI with a cookie ready to be set.
pub async fn complete(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let code = request
    .url()
    .query_pairs()
    .find_map(|(k, v)| if k == "code" { Some(v) } else { None })
    .ok_or(tide::Error::from_str(404, "no-code"))?;

  log::debug!("attempting to exchange code for token");

  let worker = request.state();
  let payload = AuthCodeRequest {
    code: code.into(),
    client_id: worker.auth0_configuration.client_id.clone(),
    client_secret: worker.auth0_configuration.client_secret.clone(),
    redirect_uri: worker.auth0_configuration.redirect_uri.clone(),
    grant_type: "authorization_code".into(),
  };
  let mut response = surf::post(&worker.auth0_configuration.token_uri)
    .body_json(&payload)?
    .await?;

  let token = token_from_response(&mut response)
    .await
    .ok_or(tide::Error::from_str(404, "token-exchange"))?;

  log::debug!("loaded token from response");

  // Actually load the user information from our db.
  let info = fetch_user(&worker.auth0_configuration.info_uri, token)
    .await
    .ok_or_else(|| tide::Error::from_str(404, "bad-token"))?;

  log::debug!("loaded user info for '{}'", info.sub);

  // Attempt to upsert this user into our db.
  let query = bson::doc! { "oid": info.sub.clone() };
  let users = worker.users_collection()?;
  let options = mongodb::options::FindOneAndUpdateOptions::builder()
    .upsert(true)
    .return_document(mongodb::options::ReturnDocument::After)
    .build();

  let state = crate::types::User {
    oid: info.sub.clone(),
    ..Default::default()
  };
  let user = users
    .find_one_and_update(
      query,
      bson::doc! { "$setOnInsert": bson::to_bson(&state).map_err(|error| {
          log::warn!("unable to serialize new user - {:?}", error);
          tide::Error::from_str(500, "user-failure")
        })?,
      },
      options,
    )
    .await
    .map_err(|error| {
      log::warn!("unable to create new user - {:?}", error);
      tide::Error::from_str(500, "user-failure")
    })?
    .ok_or(tide::Error::from_str(404, "missing-user"))?;

  log::info!("user pulled from db - {:?}", user);

  let jwt = super::claims::Claims::for_user(&user.oid).encode(&worker.web_configuration.session_secret)?;

  // Create a session and redirect them to the ui.
  let cookie = format!(
    "{}={}; {}",
    &worker.web_configuration.session_cookie, jwt, COOKIE_SET_FLAGS
  );
  let response = tide::Response::builder(302)
    .header("Set-Cookie", cookie)
    .header("Location", &worker.web_configuration.ui_redirect)
    .build();

  Ok(response)
}

/// Route: identify
///
/// This route attempts to load the user information from our db based on the session cookied
/// provided by the request.
pub async fn identify(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let worker = request.state();

  let user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-user")
  })?;

  log::debug!("successfully loaded user {:?}", user);

  tide::Body::from_json(&user).map(|b| tide::Response::builder(200).body(b).build())
}

/// Route: redirect
///
/// This route initiates the oauth handshake by redirecting the user's browser to the
/// oauth provider's login page.
pub async fn redirect(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let auth0 = &request.state().auth0_configuration;
  let url = http_types::Url::parse_with_params(
    &auth0.auth_uri,
    &[
      ("client_id", auth0.client_id.as_str()),
      ("redirect_uri", auth0.redirect_uri.as_str()),
      ("response_type", &"code"),
      ("scope", &"openid profile email"),
    ],
  )?;
  Ok(tide::Redirect::temporary(url.to_string()).into())
}
