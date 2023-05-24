use serde::{Deserialize, Serialize};

use super::ownership;

/// A request to rename a device.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceRenameRequest {
  /// the device.
  pub device_id: String,
  /// the name.
  pub new_name: String,
}

/// The individual kinds of jobs.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RegistrarJobKind {
  /// A job queued to request ownership.
  Ownership(ownership::DeviceOwnershipRequest),

  /// Renaming devices can be expensive; it is a job.
  Rename(DeviceRenameRequest),
}

/// The job container exposed by this module.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegistrarJob {
  /// A unique id for this request.
  pub(crate) id: String,

  /// The inner job type.
  pub(super) job: RegistrarJobKind,
}

impl RegistrarJob {
  /// Builds a request for taking device ownership.
  pub fn rename_device<S>(device_id: S, new_name: S) -> Self
  where
    S: std::convert::AsRef<str>,
  {
    let id = uuid::Uuid::new_v4().to_string();
    let new_name = new_name.as_ref().to_string();
    let device_id = device_id.as_ref().to_string();
    Self {
      id,
      job: RegistrarJobKind::Rename(DeviceRenameRequest { device_id, new_name }),
    }
  }

  /// Builds a request for taking device ownership.
  pub fn device_ownership<S>(user_id: S, device_id: S) -> Self
  where
    S: std::convert::AsRef<str>,
  {
    let id = uuid::Uuid::new_v4().to_string();
    let user_id = user_id.as_ref().to_string();
    let device_id = device_id.as_ref().to_string();
    Self {
      id,
      job: RegistrarJobKind::Ownership(ownership::DeviceOwnershipRequest { user_id, device_id }),
    }
  }
}
