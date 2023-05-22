use serde::{Deserialize, Serialize};

/// The type job job that will attempt to set the device authority record as owned.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceOwnershipRequest {
  /// The id of the device in question.
  pub(super) device_id: String,

  /// The id of the user in question.
  pub(super) user_id: String,
}
