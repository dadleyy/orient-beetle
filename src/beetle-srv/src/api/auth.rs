//! Note: currently considering migrating away from auth0 to google entirely.

/// Google apis.
pub mod google;

/// The flags that will be used to set our cookie when not using https.
#[cfg(debug_assertions)]
const COOKIE_SET_FLAGS: &str = "Max-Age=86400; Path=/; SameSite=Strict; HttpOnly";

/// The flags that will be used to set our cookie when using https.
#[cfg(not(debug_assertions))]
const COOKIE_SET_FLAGS: &str = "Max-Age=86400; Path=/; SameSite=Strict; HttpOnly; Secure";

/// The flags of our `Set-Cookie` header used to clear the cookie.
#[cfg(debug_assertions)]
const COOKIE_CLEAR_FLAGS: &str = "Max-Age: 0; Path=/; SameSite=Strict; HttpOnly";
/// The flags of our `Set-Cookie` header used to clear the cookie.
#[cfg(not(debug_assertions))]
const COOKIE_CLEAR_FLAGS: &str = "Max-Age: 0; Path=/; SameSite=Strict; HttpOnly; Secure";

/// Route: logout
///
/// A simple redirect with a cookie-clearing header.
pub async fn logout(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let worker = request.state();

  log::debug!("redirecting user with logout cookie");

  let cookie = format!(
    "{}=''; {}; Domain={}",
    &worker.web_configuration.session_cookie, COOKIE_CLEAR_FLAGS, &worker.web_configuration.cookie_domain,
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

  log::trace!("successfully loaded user {:?}", user);

  tide::Body::from_json(&user).map(|b| tide::Response::builder(200).body(b).build())
}
