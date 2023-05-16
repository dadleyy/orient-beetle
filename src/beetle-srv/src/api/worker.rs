use async_std::sync::{Arc, Mutex};
use serde::Serialize;
use std::io::{Error, ErrorKind, Result};

/// The type shared by all web worker requests.
#[derive(Clone)]
pub struct Worker {
  /// The original web configuration.
  pub(super) web_configuration: super::WebConfiguration,
  /// The original redis configuration.
  pub(super) redis_configuration: crate::config::RedisConfiguration,
  /// The original auth0 configuration.
  pub(super) auth0_configuration: crate::config::Auth0Configuration,

  /// Our shared mongo client + configuration.
  mongo: (mongodb::Client, crate::config::MongoConfiguration),

  /// The redis TCP connection. This is not a "pool" just yet; we're currently only using a single
  /// tcp connection across all connections.
  redis_pool: Arc<Mutex<Option<async_tls::client::TlsStream<async_std::net::TcpStream>>>>,
}

impl Worker {
  /// Builds a worker from the configuration provided by our crate.
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

  /// Will attempt to queue a render request.
  pub(super) async fn queue_render<S>(
    &self,
    device_id: &String,
    user_id: &String,
    layout: crate::rendering::RenderVariant<S>,
  ) -> Result<String>
  where
    S: AsRef<str> + Serialize,
  {
    let mut retries = 1;
    let mut id = None;

    while retries > 0 && id.is_none() {
      log::info!("received request to queue a render for device '{device_id}' from '{user_id}' (attempt {retries})");
      retries -= 1;
      let mut redis_connection = self.get_redis_lock().await?;

      if let Some(ref mut connection) = *redis_connection {
        let mut queue = crate::rendering::queue::Queue::new(connection);

        let result = queue
          .queue(
            device_id,
            &crate::rendering::queue::QueuedRenderAuthority::User(user_id.clone()),
            layout,
          )
          .await?;

        id = Some(result.0);
        break;
      }
    }

    id.ok_or_else(|| Error::new(ErrorKind::Other, "unable to queue within reasonable amount of attempts"))
  }

  /// Attempts to execute a command against the redis instance.
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

          let mut connection = crate::redis::connect(&self.redis_configuration)
            .await
            .map_err(|error| {
              log::warn!("unable to connect to redis from previous disconnect - {error}");
              error
            })?;

          result = kramer::execute(&mut connection, command).await;
          Some(connection)
        }
      };

      // TODO: add a redis connection retry configuration value that can be used here.
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
          retry_count += 1;
          lock_result.take();
          continue 'retries;
        }
      }
    }

    result
  }

  /// Given a request, this method will attempt to determine what kind of authority we are
  /// processing with.
  ///
  /// TODO: back this with redis for a more secure + controllable session store. For now
  /// we are ultimately relying on the json web token secret to prevent spoofing.
  pub(super) async fn request_authority(&self, request: &tide::Request<Self>) -> Result<Option<crate::types::User>> {
    let oid = request
      .cookie(&self.web_configuration.session_cookie)
      .and_then(|cook| super::claims::Claims::decode(cook.value(), &self.web_configuration.session_secret).ok())
      .map(|claims| claims.oid)
      .unwrap_or_default();

    if oid.is_empty() {
      return Ok(None);
    }

    log::trace!("attempting to identify via {:?}", oid);
    let users = self.users_collection()?;
    let query = bson::doc! { "oid": oid.clone() };

    users.find_one(query, None).await.map_err(|error| {
      log::warn!("unable to find - user matching '{oid}' - {error}");
      Error::new(ErrorKind::Other, "missing-user")
    })
  }

  /// Wraps the mongodb client and returns our collection.
  pub(super) fn device_diagnostic_collection(&self) -> Result<mongodb::Collection<crate::types::DeviceDiagnostic>> {
    Ok(
      self
        .mongo
        .0
        .database(&self.mongo.1.database)
        .collection(&self.mongo.1.collections.device_diagnostics),
    )
  }

  /// Wraps the mongodb client and returns our collection.
  pub(super) fn users_collection(&self) -> Result<mongodb::Collection<crate::types::User>> {
    Ok(
      self
        .mongo
        .0
        .database(&self.mongo.1.database)
        .collection(&self.mongo.1.collections.users),
    )
  }

  /// Attempts to aquire a lock, filling the contents with either a new connection, or just
  /// re-using the existing one.
  async fn get_redis_lock(
    &self,
  ) -> Result<async_std::sync::MutexGuard<'_, Option<async_tls::client::TlsStream<async_std::net::TcpStream>>>> {
    let mut lock_result = self.redis_pool.lock().await;

    match lock_result.take() {
      Some(connection) => {
        *lock_result = Some(connection);
        Ok(lock_result)
      }
      None => {
        log::warn!("no existing redis connection, establishing now");

        let connection = crate::redis::connect(&self.redis_configuration)
          .await
          .map_err(|error| {
            log::warn!("unable to connect to redis from previous disconnect - {error}");
            error
          })?;

        *lock_result = Some(connection);

        Ok(lock_result)
      }
    }
  }
}
