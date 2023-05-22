use serde::{Deserialize, Serialize};

/// TODO: this is currently a dumping ground of non-interesting struct definitions
/// for things sent over the wire or persisted in mongo.

/// Our user record. Stores minimal information; Auth0 is responsible for holding onto all
/// personally identifiable information.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct User {
  /// The oauth ID of this user.
  pub oid: String,

  /// An avatar for this user to render.
  pub picture: String,

  /// A list of device ids this user has access to.
  pub devices: Option<std::collections::HashMap<String, u8>>,
}

/// Mongo + serde + chrono don't work perfectly together; for now these are serialized into a user
/// readable string.
fn format_datetime(datetime: &chrono::DateTime<chrono::Utc>) -> String {
  format!("{}", datetime.format("%b %d, %Y %H:%M:%S"))
}

/// The various kinds of authority models supported for devices.
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceAuthorityModel {
  /// When a device is in an exclusive authority model, only a single user can manage it.
  Exclusive(String),

  /// When a device is in a shared authority model, a list ofusers can manage it.
  Shared(String, Vec<String>),
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

/// This type represents the different states of "registration" a device may be in.
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

  /// An accumulated total of messages that have been added to this device's queue.
  pub sent_message_count: Option<u32>,

  /// A list of the most recent messages that have been sent to the device.
  pub sent_messages: Option<Vec<crate::rendering::queue::QueuedRender<String>>>,

  /// The state of this device's registration.
  pub registration_state: Option<DeviceDiagnosticRegistration>,
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
