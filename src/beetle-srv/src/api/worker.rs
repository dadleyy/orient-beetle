use async_std::sync::{Arc, Mutex};
use std::io::{Error, ErrorKind, Result};

#[derive(Clone)]
pub struct Worker {
  pub(super) web_configuration: super::WebConfiguration,
  pub(super) redis_configuration: crate::config::RedisConfiguration,
  pub(super) auth0_configuration: crate::config::Auth0Configuration,
  mongo: (mongodb::Client, crate::config::MongoConfiguration),

  redis_pool: Arc<Mutex<Option<async_tls::client::TlsStream<async_std::net::TcpStream>>>>,
}

impl Worker {
  pub async fn from_config(config: super::Configuration) -> Result<Self> {
    // Attempt to connect to mongo early.
    let mongo_options = mongodb::options::ClientOptions::parse(&config.mongo.url)
      .await
      .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    let mongo = mongodb::Client::with_options(mongo_options)
      .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    let redis = crate::redis::connect(&config.redis).await?;

    let redis_pool = Arc::new(Mutex::new(Some(redis)));

    Ok(Self {
      web_configuration: config.web,
      redis_configuration: config.redis,
      auth0_configuration: config.auth0,
      mongo: (mongo, config.mongo),
      redis_pool,
    })
  }

  async fn redis(&self) -> Result<async_tls::client::TlsStream<async_std::net::TcpStream>> {
    crate::redis::connect(&self.redis_configuration).await
  }

  pub(super) async fn command<S, V>(&self, command: &kramer::Command<S, V>) -> Result<kramer::Response>
  where
    S: std::fmt::Display,
    V: std::fmt::Display,
  {
    let mut now = std::time::Instant::now();
    let mut lock_result = self.redis_pool.lock().await;
    log::trace!("redis 'pool' lock in in {}ms", now.elapsed().as_millis());
    now = std::time::Instant::now();

    #[allow(unused_assignments)]
    let mut result = Err(Error::new(ErrorKind::Other, "failed send"));

    let mut retry_count = 0;

    'retries: loop {
      *lock_result = match lock_result.take() {
        Some(mut connection) => {
          log::trace!("redis lock taken in in {}ms", now.elapsed().as_millis());
          result = kramer::execute(&mut connection, command).await;
          Some(connection)
        }
        None => {
          log::warn!("no existing redis connection, establishing now");

          let mut connection = self.redis().await.map_err(|error| {
            log::warn!("unable to connect to redis from previous disconnect - {error}");
            error
          })?;

          result = kramer::execute(&mut connection, command).await;
          Some(connection)
        }
      };

      if retry_count > 0 {
        log::warn!("exceeded redis retry count, breaking with current result");
        break;
      }

      match result {
        // If we were successful, there is nothing more to do here, exit the loop
        Ok(_) => break,

        // If we failed due to a broken pipe, clear out our connection and try one more time.
        Err(error) if error.kind() == ErrorKind::BrokenPipe => {
          log::warn!("detected broken pipe, re-trying");
          retry_count += 1;
          lock_result.take();
          continue 'retries;
        }

        Err(error) => {
          log::warn!("redis command failed for ({:?}) ({:?}), no retry", error, error.kind());
          return Err(error);
        }
      }
    }

    result
  }

  pub(super) async fn request_authority(&self, request: &tide::Request<Self>) -> Result<Option<crate::types::User>> {
    let oid = request
      .cookie(&self.web_configuration.session_cookie)
      .and_then(|cook| super::claims::Claims::decode(cook.value(), &self.web_configuration.session_secret).ok())
      .map(|claims| claims.oid)
      .unwrap_or_default();

    if oid.len() == 0 {
      return Ok(None);
    }

    log::debug!("attempting to identify via {:?}", oid);
    let users = self.users_collection()?;
    let query = bson::doc! { "oid": oid.clone() };

    users.find_one(query, None).await.map_err(|error| {
      log::warn!("unable to create new player - {:?}", error);
      Error::new(ErrorKind::Other, format!("bad-query - {error}"))
    })
  }

  pub(super) fn device_diagnostic_collection(&self) -> Result<mongodb::Collection<crate::types::DeviceDiagnostic>> {
    Ok(
      self
        .mongo
        .0
        .database(&self.mongo.1.database)
        .collection(&self.mongo.1.collections.device_diagnostics),
    )
  }

  pub(super) fn users_collection(&self) -> Result<mongodb::Collection<crate::types::User>> {
    Ok(
      self
        .mongo
        .0
        .database(&self.mongo.1.database)
        .collection(&self.mongo.1.collections.users),
    )
  }
}
