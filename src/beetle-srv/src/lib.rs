use std::fmt;

pub mod constants;

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
}

impl fmt::Display for IndexedDevice {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let mins = chrono::Utc::now().signed_duration_since(self.last_seen).num_seconds();
    write!(formatter, "[{}]: last seen @ {} seconds ago", self.id, mins)
  }
}
