//! TODO: this is currently a dumping ground of non-interesting struct definitions
//! for things sent over the wire or persisted in mongo.

use serde::{Deserialize, Serialize};

/// The device state is a bit beefy.
mod device_state;
pub use device_state::{DeviceRenderingState, DeviceRenderingStateMessageEntry, DeviceState, DeviceStateMessageOrigin};

/// The general schema related to the background jobs used.
pub(crate) mod jobs;

/// The "snapshot in time" of device information we want stored on our user documents themselves.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct UserDeviceSnapshot {
  /// The nickname of the device.
  pub nickname: Option<String>,
}

/// Our user record. Stores minimal information; Auth0 is responsible for holding onto all
/// personally identifiable information.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct User {
  /// The oauth ID of this user.
  pub oid: String,

  /// An avatar for this user to render.
  pub picture: String,

  /// The name of this user, per original oauth source.
  pub name: Option<String>,

  /// The user-preferred nickname.
  pub nickname: Option<String>,

  /// A list of device ids this user has access to.
  pub devices: Option<std::collections::HashMap<String, UserDeviceSnapshot>>,
}

/// Mongo + serde + chrono don't work perfectly together; for now these are serialized into a user
/// readable string.
fn format_datetime(datetime: &chrono::DateTime<chrono::Utc>) -> String {
  datetime.format("%b %d, %Y %H:%M:%S").to_string()
}

/// The various kinds of authority models supported for devices.
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceAuthorityModel {
  /// When a device is in an exclusive authority model, only a single user can manage it.
  Exclusive(String),

  /// When a device is in a shared authority model, a list of users can manage it.
  Shared(String, Vec<String>),

  /// When a device is in an "open" model, anyone can send things to it. We will retain the list of
  /// folks who have added themselves as a way to transition easily into "shared".
  Public(String, Vec<String>),
}

/// The schema of our records that are stored in `device_histories` collection.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeviceHistoryRecord {
  /// The id of a device.
  pub(crate) device_id: String,
  /// This list of all renders for this device.
  pub(crate) render_history: Option<Vec<crate::rendering::queue::QueuedRender<String>>>,
}

/// The schema of our records that are stored in `device_authorities` collection.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeviceAuthorityRecord {
  /// The id of a device.
  pub(crate) device_id: String,

  /// The model.
  pub(crate) authority_model: Option<DeviceAuthorityModel>,
}

/// At the end of the day, what devices a user has access to is still being maintained on the user
/// record (for efficiency of permission checks).
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeviceDiagnosticOwnership {
  /// The if of the user that first registered this device.
  pub original_owner: String,
}

/// This type represents the different states of "registration" a device may be in. This
/// information is long-lived in the `device-diagnostics` collection. Note that the registration is
/// not the same as the `AuthorityModel` associated with a device: the registration information is
/// generally immutable.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceDiagnosticRegistration {
  /// The state where we have seen the device, but nobody has claimed it yet.
  #[default]
  Initial,

  /// The state where we have heard from the device, and sent it the initial image.
  PendingRegistration,

  /// The state where some user has claimed a device.
  Owned(DeviceDiagnosticOwnership),
}

/// The different kinds of things that can happen on a schedule for a device.
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceScheduleKind {
  /// The most basic kind of schedule.
  UserEventsBasic(String),
}

/// A schedule of things to render for a specific device.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeviceSchedule {
  /// The id of a device for this schedule.
  pub device_id: String,

  /// The timestamp of the last executed attempt.
  pub last_executed: Option<u64>,

  /// The underlying schedule implementation.
  pub kind: Option<DeviceScheduleKind>,
}

/// This type is serialized into our mongoDB instance for every device and updated periodically
/// as the device communicates with the server.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeviceDiagnostic {
  /// The id of this device.
  pub id: String,

  /// The first timestamp we received an `LPUSH` into our update queue from this device.
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  pub first_seen: Option<chrono::DateTime<chrono::Utc>>,

  /// The last timestamp we received an `LPUSH` into our update queue from this device.
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  pub last_seen: Option<chrono::DateTime<chrono::Utc>>,

  /// This device nickname.
  pub nickname: Option<String>,

  /// An accumulated total of messages that have been added to this device's queue.
  pub sent_message_count: Option<u32>,

  /// The state of this device's registration.
  pub registration_state: Option<DeviceDiagnosticRegistration>,
}

impl DeviceDiagnostic {
  /// Creates a snapshot for persistence on user records themselves.
  pub fn snapshot(&self) -> UserDeviceSnapshot {
    UserDeviceSnapshot {
      nickname: self.nickname.as_ref().cloned(),
    }
  }
}

impl std::fmt::Display for DeviceDiagnostic {
  fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    let start = self
      .first_seen
      .map(|time| format_datetime(&time))
      .unwrap_or_else(|| "n/a".to_string());
    let end = self
      .last_seen
      .map(|time| format_datetime(&time))
      .unwrap_or_else(|| "n/a".to_string());

    let last = self
      .last_seen
      .map(|time| chrono::Utc::now().signed_duration_since(time).num_seconds())
      .unwrap_or(-1);

    let marker = match last {
      0..=30 => '+',
      _ => '!',
    };

    write!(
      formatter,
      "[{}] {} ({last:05} seconds ago). {start} -> {end}",
      marker, self.id
    )
  }
}
