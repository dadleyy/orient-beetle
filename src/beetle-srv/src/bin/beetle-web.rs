use std::io::{Error, ErrorKind, Result};

#[derive(Default)]
struct CommandLineConfig {
  message: Option<String>,
  redis: (String, String, String),
}

async fn run(mut config: CommandLineConfig) -> Result<()> {
  let connection = async_std::net::TcpStream::connect(format!("{}:{}", config.redis.0, config.redis.1)).await?;
  log::info!("connection established, negotiating tls");
  let connector = async_tls::TlsConnector::default();
  let mut stream = connector.connect(&config.redis.0, connection).await?;
  log::info!("stream ready");
  match kramer::execute(
    &mut stream,
    kramer::Command::Auth::<&str, &str>(kramer::AuthCredentials::Password(&config.redis.2)),
  )
  .await
  {
    Ok(thing) => log::info!("auth result - {thing:?}"),
    Err(error) => log::info!("auth result - {error:?}"),
  };

  if let Some(message) = config.message.take() {
    log::info!("sending '{message}'");

    let out = kramer::execute(
      &mut stream,
      kramer::Command::List(kramer::ListCommand::Push(
        (kramer::Side::Left, kramer::Insertion::Always),
        "ob:m",
        kramer::Arity::One(message.as_str()),
      )),
    )
    .await;

    log::info!("message result - {out:?}");

    return Ok(());
  }

  Ok(())
}

fn main() -> Result<()> {
  dotenv::dotenv().map_err(|error| Error::new(ErrorKind::Other, error))?;
  env_logger::init();

  log::info!("environment + logger ready.");

  let redis = std::env::var("REDIS_HOST")
    .ok()
    .zip(std::env::var("REDIS_PORT").ok())
    .zip(std::env::var("REDIS_AUTH").ok())
    .map(|((h, p), a)| (h, p, a));

  let mut config = CommandLineConfig::default();
  if let Some(redis) = redis {
    config.redis = redis;
  }

  if let Some(message) = std::env::args().skip(1).next() {
    config.message = Some(message);
  }

  async_std::task::block_on(run(config))
}
