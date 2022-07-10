use serde::{Deserialize, Serialize};

/// TODO: this is currently a dumping ground of non-interesting struct definitions
/// for things sent over the wire or persisted in mongo.

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct User {
  pub oid: String,
  pub devices: Option<std::collections::HashMap<String, u8>>,
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
