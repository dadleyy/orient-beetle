use std::io::{Error, ErrorKind, Result};

pub async fn connect<S>(host: S, port: S, auth: S) -> Result<async_tls::client::TlsStream<async_std::net::TcpStream>>
where
  S: AsRef<str>,
{
  let connector = async_tls::TlsConnector::default();
  let mut stream = connector
    .connect(
      host.as_ref(),
      async_std::net::TcpStream::connect(format!("{}:{}", host.as_ref(), port.as_ref())).await?,
    )
    .await?;

  let auth_result = kramer::execute(
    &mut stream,
    kramer::Command::Auth::<&str, bool>(kramer::AuthCredentials::Password(auth.as_ref())),
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
