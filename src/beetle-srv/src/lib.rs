use std::fmt;

pub mod api;
pub mod config;
pub mod constants;
pub mod identity;
pub mod mongo;
pub mod redis;
pub mod registrar;
pub mod types;

#[derive(Debug)]
pub struct IndexedDevice {
  id: String,
  last_seen: chrono::DateTime<chrono::Utc>,
}

impl IndexedDevice {
  pub fn from_redis<S>(id: S, date: S) -> Option<Self>
  where
    S: AsRef<str>,
  {
    let dt = chrono::DateTime::parse_from_rfc3339(date.as_ref()).ok()?;
    Some(Self {
      id: id.as_ref().to_string(),
      last_seen: dt.with_timezone(&chrono::Utc),
    })
  }

  pub fn id(&self) -> &String {
    &self.id
  }

  pub fn last_seen(&self) -> &chrono::DateTime<chrono::Utc> {
    &self.last_seen
  }
}

impl fmt::Display for IndexedDevice {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let mins = chrono::Utc::now().signed_duration_since(self.last_seen).num_seconds();
    write!(formatter, "[{}]: last seen @ {} seconds ago", self.id, mins)
  }
}
