use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct RegistrationPayload {
  device_id: String,
}

#[derive(Debug, Serialize)]
struct RegistrationResponse {
  id: String,
}

/// Route: register
///
/// This api route will attempt to parse the request payload and register the device id
/// with the user identified in the request cookie.
pub async fn register(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let worker = request.state();

  let user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-player")
  })?;

  let payload = request.body_json::<RegistrationPayload>().await.map_err(|error| {
    log::warn!("invalid request payload - {error}");
    tide::Error::from_str(422, "bad-payload")
  })?;

  log::debug!("user {user:?} requesting {payload:?}");

  tide::Body::from_json(&RegistrationResponse { id: payload.device_id })
    .map(|body| tide::Response::builder(200).body(body).build())
}
