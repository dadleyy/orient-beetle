use serde::{Deserialize, Serialize};

/// TODO: this is currently a dumping ground of non-interesting struct definitions
/// for things sent over the wire or persisted in mongo.

/// Our user record. Stores minimal information; Auth0 is responsible for holding onto all
/// personally identifiable information.
#[derive(Deserialize, Serialize, Debug, Default)]
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

/// This type is serialized into our mongoDB instance for every device and updated periodically
/// as the device communicates with the server.
#[derive(Deserialize, Serialize, Debug, Default)]
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
