use serde::{Deserialize, Serialize};
use std::io;

use super::ownership;
use super::rename::DeviceRenameRequest;

/// The enumerated result set of all background jobs.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum JobResult {
  /// The job is currently pending.
  Pending,

  /// A success without any more info.
  Success,

  /// A failure with a reason.
  Failure(String),
}

/// Rendering jobs specific to the registrar. Eventually this might be expanded to wrap _all_
/// rendering jobs that currently go directly to the queue.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RegistrarRenderKinds {
  /// Queues a render for the initial scannable
  RegistrationScannable(String),
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

  /// Render jobs specific to the registrar.
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
  pub(super) job: RegistrarJobKind,
}

impl RegistrarJob {
  /// Serializes and encrypts a job.
  pub fn encrypt(self, config: &crate::config::RegistrarConfiguration) -> io::Result<String> {
    let header = &jsonwebtoken::Header::default();
    let secret = jsonwebtoken::EncodingKey::from_secret(config.vendor_api_secret.as_bytes());
    let exp = chrono::Utc::now()
      .checked_add_signed(chrono::Duration::minutes(1440))
      .unwrap_or_else(chrono::Utc::now)
      .timestamp() as u32;

    jsonwebtoken::encode(header, &RegistrarJobEncrypted { exp, job: self }, &secret)
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to encrypt job - {error}")))
  }

  /// Will attempt to store an access token for a user.
  pub fn access_token_refresh(handle: crate::vendor::google::TokenHandle, user_id: String) -> Self {
    let id = uuid::Uuid::new_v4().to_string();
    Self {
      id,
      job: RegistrarJobKind::UserAccessTokenRefresh { handle, user_id },
    }
  }

  /// Builds a request for toggling ownership record model type.
  pub fn set_public_availability<S>(device_id: S, transition: ownership::PublicAvailabilityChange) -> Self
  where
    S: std::convert::AsRef<str>,
  {
    let id = uuid::Uuid::new_v4().to_string();
    let device_id = device_id.as_ref().to_string();

    Self {
      id,
      job: RegistrarJobKind::OwnershipChange(ownership::DeviceOwnershipChangeRequest::SetPublicAvailability(
        device_id, transition,
      )),
    }
  }

  /// Builds a request for taking device ownership.
  pub fn registration_scannable<S>(device_id: S) -> Self
  where
    S: std::convert::AsRef<str>,
  {
    let id = uuid::Uuid::new_v4().to_string();
    let device_id = device_id.as_ref().to_string();
    Self {
      id,
      job: RegistrarJobKind::Renders(RegistrarRenderKinds::RegistrationScannable(device_id)),
    }
  }

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
