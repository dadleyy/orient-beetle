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
      None => {
        let stream = async_std::net::TcpStream::connect(format!("{}:{}", self.redis.0, self.redis.1)).await?;
        let connector = async_tls::TlsConnector::default();
        let mut connect = connector.connect(&self.redis.0, stream).await?;
        log::debug!("successfully connected, attempting auth");

        kramer::execute(
          &mut connect,
          kramer::Command::Auth::<&str, bool>(kramer::AuthCredentials::Password(&self.redis.2)),
        )
        .await?;

        Some(connect)
      }

      Some(mut inner) => {
        let output = kramer::execute(
          &mut inner,
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

        if should_send {
          let ids = (0..10)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect::<Vec<String>>();

          log::debug!("populating ids - {:?}", ids);

          let insertion = kramer::execute(
            &mut inner,
            kramer::Command::List(kramer::ListCommand::Push(
              (kramer::Side::Left, kramer::Insertion::Always),
              beetle::constants::REGISTRAR_AVAILABLE,
              kramer::Arity::Many(ids),
            )),
          )
          .await?;

          log::debug!("insertion result - {:?}", insertion);
        }

        let taken = kramer::execute(
          &mut inner,
          kramer::Command::List::<&str, bool>(kramer::ListCommand::Pop(
            kramer::Side::Left,
            beetle::constants::REGISTRAR_INCOMING,
            None,
          )),
        )
        .await?;

        log::trace!("taken ids - {:?}", taken);

        if let kramer::Response::Item(kramer::ResponseValue::String(id)) = taken {
          log::debug!("device submitted registration for id '{}'", id);

          let activation = kramer::execute(
            &mut inner,
            kramer::Command::Hashes(kramer::HashCommand::Set(
              beetle::constants::REGISTRAR_ACTIVE,
              kramer::Arity::One((id.as_str(), chrono::Utc::now().to_rfc3339())),
              kramer::Insertion::Always,
            )),
          )
          .await?;

          log::debug!("device activation - {:?}", activation);

          let setter = kramer::Command::Sets(kramer::SetCommand::Add(
            beetle::constants::REGISTRAR_INDEX,
            kramer::Arity::One(id.as_str()),
          ));
          let activation = kramer::execute(&mut inner, setter).await?;

          log::debug!("device indexing - {:?}", activation);
        }

        Some(inner)
      }
    };

    Ok(())
  }
}

async fn run(config: CommandLineConfig) -> Result<()> {
  let mut worker = Worker::new(config);
  let mut failures = Vec::with_capacity(3);
  let mut interval = async_std::stream::interval(std::time::Duration::from_secs(1));

  while failures.len() < 10 {
    interval.next().await;
    log::debug!("attempting worker frame");

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
