use std::io::{Error, ErrorKind, Result};

pub async fn connect(
  config: &crate::config::RedisConfiguration,
) -> Result<async_tls::client::TlsStream<async_std::net::TcpStream>> {
  let connector = async_tls::TlsConnector::default();
  let mut stream = connector
    .connect(
      &config.host,
      async_std::net::TcpStream::connect(format!("{}:{}", config.host, config.port)).await?,
    )
    .await?;

  let auth_result = kramer::execute(
    &mut stream,
    kramer::Command::Auth::<&str, bool>(kramer::AuthCredentials::Password(&config.auth)),
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
