//! This module is currently in the process of replacing the Auth0-based module defined in the
//! parent directory. Some of the code in here is repetetive while that is being phases out.

use crate::{registrar, schema};
use anyhow::Context;

/// This value is how auth0 "tags" ids during its oauth handshake. It will be added for all users
/// authenticating through google for backwards-compatibility.
const GOOGLE_ID_PREFIX: &str = "google-oauth2|";

/// Route: google redirect start.
pub async fn redirect(request: tide::Request<crate::api::Worker>) -> tide::Result {
  let mut url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").with_context(|| "bad")?;
  let state = request.state();
  {
    let mut query = url.query_pairs_mut();
    query.append_pair("client_id", &state.google_configuration.client_id);
    query.append_pair("redirect_uri", &state.google_configuration.redirect_uri);
    query.append_pair("scope", state.google_configuration.scopes.join(" ").as_str());
    query.append_pair("access_type", "offline");
    query.append_pair("prompt", "consent");
    query.append_pair("response_type", "code");
  }
  log::debug!("the redirect - {url}");
  Ok(tide::Redirect::new(url).into())
}

/// Route: google redirect finish.
pub async fn complete(request: tide::Request<crate::api::Worker>) -> tide::Result {
  let query = request.query::<crate::vendor::google::CodeQuery>()?;
  let worker = request.state();
  log::trace!("have code - '{}'", query.code);

  let mut response = surf::post("https://oauth2.googleapis.com/token")
    .body_json(&crate::vendor::google::TokenRequest {
      code: query.code,
      client_id: worker.google_configuration.client_id.clone(),
      client_secret: worker.google_configuration.client_secret.clone(),
      redirect_uri: worker.google_configuration.redirect_uri.clone(),
      grant_type: "authorization_code",
    })?
    .await?;

  let status = response.status();

  if status != 200 {
    log::warn!("received non-200 status from code exchange - '{status}'");
  }

  let body_string = response.body_string().await?;
  let parsed = serde_json::from_str::<crate::vendor::google::TokenResponse>(&body_string)?;
  log::debug!("loaded token - {parsed:?}");

  let handle = crate::vendor::google::TokenHandle {
    created: chrono::Utc::now(),
    token: parsed,
  };
  let userinfo = crate::vendor::google::fetch_user(&handle).await?;

  let normalized_id = format!("{GOOGLE_ID_PREFIX}{}", userinfo.id);
  let query = bson::doc! { "oid": &normalized_id };
  let options = mongodb::options::FindOneAndUpdateOptions::builder()
    .upsert(true)
    .return_document(mongodb::options::ReturnDocument::After)
    .build();

  // TODO: this got a little bit messy when names were introduced: we're attempting to upsert a
  // user if it doesnt exist, but _always_ updating the `name` that came back from google user
  // info.
  let state = schema::User {
    oid: normalized_id.clone(),
    picture: userinfo.picture.clone(),
    name: Some(userinfo.name.clone()),
    ..Default::default()
  };

  log::debug!("loaded user info - '{}' ({:?})", state.oid, state.name);

  let upsert_doc = bson::to_bson(&state).map_err(|error| {
    log::warn!("unable to serialize new user - {:?}", error);
    tide::Error::from_str(500, "user-failure")
  })?;

  let users = worker.users_collection()?;
  let user = users
    .find_one_and_update(query.clone(), bson::doc! { "$setOnInsert": upsert_doc }, options)
    .await
    .map_err(|error| {
      log::warn!("unable to create new user - {:?}", error);
      tide::Error::from_str(500, "user-failure")
    })?
    .ok_or_else(|| tide::Error::from_str(404, "missing-user"))?;

  // TODO(name-migration): remove this after some time has passed for stabilization. We weren't
  // originally recording names until we started rendering events with user names.
  users
    .update_one(query, bson::doc! { "$set": { "name": state.name } }, None)
    .await?;

  log::debug!("loaded user from database - '{}'", user.oid);
  let jwt = crate::api::claims::Claims::for_user(&user.oid).encode(&worker.web_configuration.session_secret)?;

  if let Err(error) = worker
    .queue_job_kind(registrar::RegistrarJobKind::UserAccessTokenRefresh {
      handle,
      user_id: user.oid,
    })
    .await
  {
    log::warn!("unable to queued refresh token request - {error}");
  }

  // Create a session and redirect them to the ui.
  let cookie = format!(
    "{}={}; {}; Domain={}",
    &worker.web_configuration.session_cookie,
    jwt,
    super::COOKIE_SET_FLAGS,
    &worker.web_configuration.cookie_domain,
  );

  log::info!("sending cookie through redirect - '{cookie}'");

  let response = tide::Response::builder(302)
    .header("Set-Cookie", cookie)
    .header("Location", &worker.web_configuration.ui_redirect)
    .build();

  Ok(response)
}
