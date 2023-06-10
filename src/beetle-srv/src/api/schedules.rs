//! This module defines the api routes for updating a device schedule; this is how the ui will
//! configure the registrar to periodically render various scheduled things to a given device.

use serde::{Deserialize, Serialize};

/// Defines the schema of the url query for our lookup.
#[derive(Deserialize, Serialize)]
struct DeviceScheduleLookupQuery {
  #[allow(clippy::missing_docs_in_private_items)]
  device_id: String,
}

/// Requests that we either update or create a device schedule for a given device.
pub(super) async fn update(request: tide::Request<super::Worker>) -> tide::Result {
  super::Worker::require_authority(&request).await?;
  Ok("".into())
}

/// Requests a lookup for a schedule based on a device id.
pub(super) async fn find(request: tide::Request<super::Worker>) -> tide::Result {
  super::Worker::require_authority(&request).await?;
  Ok("".into())
}
