use async_std::prelude::*;
use std::io::{Error, ErrorKind, Result};

#[derive(Default)]
struct CommandLineConfig {
  redis: (String, String, String),
}

struct Worker {
  redis: (String, String, String),
  connection: Option<async_tls::client::TlsStream<async_std::net::TcpStream>>,
}

/// The main thing our worker will be responsible for is to count the amount of available ids
/// in our pool that devices will pull down to identify themselves. If that amount reaches a
/// quantity below a specific threshold, fill it back up.
async fn fill_pool(mut stream: &mut async_tls::client::TlsStream<async_std::net::TcpStream>) -> Result<usize> {
  let output = kramer::execute(
    &mut stream,
    kramer::Command::List::<&str, bool>(kramer::ListCommand::Len(beetle::constants::REGISTRAR_AVAILABLE)),
  )
  .await?;

  let should_send = match output {
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) if amount < 10 => {
      log::debug!("not enough ids, populating");
      true
    }
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) => {
      log::trace!("nothing to do, plenty of ids ('{amount}')");
      false
    }
    other => {
      log::warn!("unexpected response from count: {:?}", other);
      false
    }
  };

  if should_send == false {
    return Ok(0);
  }

  let ids = (0..10)
    .map(|_| uuid::Uuid::new_v4().to_string())
    .collect::<Vec<String>>();
  let count = ids.len();

  log::info!("populating ids - {:?}", ids);

  let insertion = kramer::execute(
    &mut stream,
    kramer::Command::List(kramer::ListCommand::Push(
      (kramer::Side::Left, kramer::Insertion::Always),
      beetle::constants::REGISTRAR_AVAILABLE,
      kramer::Arity::Many(ids),
    )),
  )
  .await?;

  log::debug!("insertion result - {:?}", insertion);

  Ok(count)
}

/// The second main function of our registrar is to keep our server informed of the active devices
/// by pulling off a queue that is pushed to by devices during regular operating procedure. With an
/// id pulled from the queue, we will store:
///
/// 1. the current timestamp in a hash of `<id> -> timestamp`
/// 2. the id we received in a `Set` for easy indexing.
async fn mark_active(mut stream: &mut async_tls::client::TlsStream<async_std::net::TcpStream>) -> Result<usize> {
  let taken = kramer::execute(
    &mut stream,
    kramer::Command::List::<&str, bool>(kramer::ListCommand::Pop(
      kramer::Side::Left,
      beetle::constants::REGISTRAR_INCOMING,
      None,
    )),
  )
  .await?;

  log::trace!("taken ids - {:?}", taken);

  if let kramer::Response::Item(kramer::ResponseValue::String(id)) = taken {
    log::debug!("device '{}' submitted registration", id);

    let activation = kramer::execute(
      &mut stream,
      kramer::Command::Hashes(kramer::HashCommand::Set(
        beetle::constants::REGISTRAR_ACTIVE,
        kramer::Arity::One((id.as_str(), chrono::Utc::now().to_rfc3339())),
        kramer::Insertion::Always,
      )),
    )
    .await?;

    log::trace!("device activation - {:?}", activation);

    let setter = kramer::Command::Sets(kramer::SetCommand::Add(
      beetle::constants::REGISTRAR_INDEX,
      kramer::Arity::One(id.as_str()),
    ));
    let activation = kramer::execute(&mut stream, setter).await?;

    log::trace!("device indexing - {:?}", activation);
  }

  Ok(0usize)
}

impl Worker {
  fn new(config: CommandLineConfig) -> Self {
    Worker {
      redis: (config.redis.0, config.redis.1, config.redis.2),
      connection: None,
    }
  }

  async fn work(&mut self) -> Result<()> {
    let stream = self.connection.take();

    self.connection = match stream {
      None => beetle::connect(&self.redis.0, &self.redis.1, &self.redis.2)
        .await
        .map(Some)?,

      Some(mut inner) => {
        let amount = fill_pool(&mut inner).await?;
        if amount > 0 {
          log::info!("filled pool with '{}' new ids", amount)
        }
        mark_active(&mut inner).await?;

        Some(inner)
      }
    };

    Ok(())
  }
}

async fn run(config: CommandLineConfig) -> Result<()> {
  let mut worker = Worker::new(config);
  let mut failures = Vec::with_capacity(3);
  let mut interval = async_std::stream::interval(std::time::Duration::from_millis(200));

  while failures.len() < 10 {
    interval.next().await;
    log::trace!("attempting worker frame");

    match worker.work().await {
      Err(error) => failures.push(format!("{error}")),
      Ok(()) => {
        if failures.len() != 0 {
          log::info!("recovered from failures: {}", failures.drain(0..).collect::<String>());
        }
      }
    }
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

  async_std::task::block_on(run(config))
}
