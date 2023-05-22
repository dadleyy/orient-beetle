use std::io::{Error, ErrorKind, Result};

/// An alias that wraps our tcp stream in TLS. This should ideally support both secure and
/// non-secure connections (for local development).
pub type RedisConnection = async_tls::client::TlsStream<async_std::net::TcpStream>;

/// Helper function to create the key that will be popped from on the device to receive the next
/// message it should display.
pub fn device_message_queue_id<S>(input: S) -> String
where
  S: std::fmt::Display,
{
  format!("ob:{input}")
}

/// Wraps the configuration we have; the only functionality beyond opening the tcp stream here is
/// an initial request to the redis instance to authenticate.
pub async fn connect(config: &crate::config::RedisConfiguration) -> Result<RedisConnection> {
  let connector = async_tls::TlsConnector::default();
  let mut stream = connector
    .connect(
      &config.host,
      async_std::net::TcpStream::connect(format!("{}:{}", config.host, config.port)).await?,
    )
    .await?;

  match &config.auth {
    Some(auth) => {
      let auth_result = kramer::execute(
        &mut stream,
        kramer::Command::Auth::<&str, bool>(kramer::AuthCredentials::Password(auth)),
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
    None => Ok(stream),
  }
}
