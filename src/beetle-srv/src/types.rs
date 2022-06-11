use serde::{Deserialize, Serialize};

/// TODO: this is currently a dumping ground of non-interesting struct definitions
/// for things sent over the wire or persisted in mongo.

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct User {
  pub(crate) oid: String,
  pub(crate) devices: Option<std::collections::HashMap<String, u8>>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct DeviceDiagnostic {
  pub(crate) id: String,
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  pub(crate) first_seen: Option<chrono::DateTime<chrono::Utc>>,
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  pub(crate) last_seen: Option<chrono::DateTime<chrono::Utc>>,
}
