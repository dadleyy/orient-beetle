use serde::{Deserialize, Serialize};
use std::io;

use super::device_state;
use super::ownership;
use super::rename::DeviceRenameRequest;

/// Rendering jobs specific to the registrar. Eventually this might be expanded to wrap _all_
/// rendering jobs that currently go directly to the queue.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RegistrarRenderKinds {
  /// Queues a render for the initial scannable
  RegistrationScannable(String),

  /// Attempts to render the current device state.
  CurrentDeviceState(String),
}

/// The individual kinds of jobs.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RegistrarJobKind {
  /// A job queued to request ownership.
  Ownership(ownership::DeviceOwnershipRequest),

  /// A job queued to request an update to the ownership model.
  OwnershipChange(ownership::DeviceOwnershipChangeRequest),

  /// Renaming devices can be expensive; it is a job.
  Rename(DeviceRenameRequest),

  /// These jobs mutate the current "rendered" device state.
  MutateDeviceState(device_state::DeviceStateTransitionRequest),

  /// An immediate attempt to run the schedule for a device.
  RunDeviceSchedule(String),

  /// A job that will simply turn on or off the default schedule for a device, given a user whose
  /// calendar would be used.
  ToggleDefaultSchedule {
    /// Whether or not to enable or disable.
    should_enable: bool,
    /// The id of a device in question.
    device_id: String,
    /// The id of a user who is claiming the default schedule.
    user_id: String,
  },

  /// Render jobs specific to the registrar. This is the job used by the UI to request that the
  /// large-form registration scannable be rendered onto the device.
  Renders(RegistrarRenderKinds),

  /// Processes a new access token for a given user.
  UserAccessTokenRefresh {
    /// The handle wrapping an access token.
    handle: crate::vendor::google::TokenHandle,
    /// The id of our user associated with this access token.
    user_id: String,
  },
}

/// The job container exposed by this module. Wrapping an underyling job inside of this allows to
/// encrypt all jobs in case they contain sensitive information.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegistrarJobEncrypted {
  /// The exp field used by jwt.
  pub(super) exp: u32,

  /// The inner job type.
  pub(super) job: RegistrarJob,
}

/// The job container exposed by this module.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegistrarJob {
  /// A unique id for this request.
  pub(crate) id: String,

  /// The inner job type.
  pub(crate) job: RegistrarJobKind,
}

impl RegistrarJob {
  /// Serializes and encrypts a job.
  pub fn encrypt(self, config: &crate::config::RegistrarConfiguration) -> io::Result<String> {
    // TODO(job_encryption): using jwt here for ease, not the fact that it is the best. The
    // original intent in doing this was to avoid having plaintext in our redis messages.
    // Leveraging and existing depedency like `aes-gcm` would be awesome.
    let header = &jsonwebtoken::Header::default();
    let secret = jsonwebtoken::EncodingKey::from_secret(config.vendor_api_secret.as_bytes());

    let exp = chrono::Utc::now()
      .checked_add_signed(chrono::Duration::minutes(1440))
      .unwrap_or_else(chrono::Utc::now)
      .timestamp() as u32;

    jsonwebtoken::encode(header, &RegistrarJobEncrypted { exp, job: self }, &secret)
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to encrypt job - {error}")))
  }
}
