use serde::{Deserialize, Serialize};

/// TODO: this is currently a dumping ground of non-interesting struct definitions
/// for things sent over the wire or persisted in mongo.

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct User {
  pub oid: String,
  pub devices: Option<std::collections::HashMap<String, u8>>,
}

fn format_datetime(datetime: &chrono::DateTime<chrono::Utc>) -> String {
  format!("{}", datetime.format("%b %d, %Y %H:%M:%S"))
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct DeviceDiagnostic {
  pub id: String,
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  pub first_seen: Option<chrono::DateTime<chrono::Utc>>,
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
  pub sent_message_count: Option<u32>,
}

impl std::fmt::Display for DeviceDiagnostic {
  fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    let start = self
      .first_seen
      .map(|time| format_datetime(&time))
      .unwrap_or("n/a".to_string());
    let end = self
      .last_seen
      .map(|time| format_datetime(&time))
      .unwrap_or("n/a".to_string());

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
