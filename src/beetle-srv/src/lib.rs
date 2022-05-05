use std::{
  fmt,
  io::{Error, ErrorKind, Result},
};

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

pub async fn connect(
  host: &String,
  port: &String,
  auth: &String,
) -> Result<async_tls::client::TlsStream<async_std::net::TcpStream>> {
  let connector = async_tls::TlsConnector::default();
  let mut stream = connector
    .connect(
      &host,
      async_std::net::TcpStream::connect(format!("{}:{}", host, port)).await?,
    )
    .await?;

  let auth_result = kramer::execute(
    &mut stream,
    kramer::Command::Auth::<&str, bool>(kramer::AuthCredentials::Password(&auth)),
  )
  .await?;

  match auth_result {
    kramer::Response::Item(kramer::ResponseValue::String(value)) if value.as_str() == "OK" => Ok(stream),
    other => {
      log::warn!("unrecognized auth response - {other:?}");
      Err(Error::new(ErrorKind::Other, "bad-auth"))
    }
  }
}
