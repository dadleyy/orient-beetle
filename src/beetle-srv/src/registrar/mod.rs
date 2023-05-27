use serde::Deserialize;
use std::io;

/// The ownership model defines types + functions associated with managing a devices ownership.
pub mod ownership;

/// This module defines functionality associated with managing the acl pool.
mod pool;

/// This module defines functionality associated with managing the acl pool.
mod diagnostics;

/// Defines the rename device job.
mod rename;

/// Defines rules for what can be done to devices.
mod access;
pub use access::{user_access, AccessLevel};

/// Just a place to put the types generally associated with background work.
pub mod jobs;
pub use jobs::{RegistrarJob, RegistrarJobKind};

/// If no value is provided in the api, this value will be used as the minimum amount of entries in
/// our pool that we need. If the current amount is less than this, we will generate ids for and
/// store them in the system.
const DEFAULT_POOL_MINIMUM: u8 = 3;

/// The configuration specific to maintaining a registration of available ids.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct RegistrarConfiguration {
  // TODO: the cli's registar configuration uses these fields, and we may as well.
  /// The auth username that will be given on burn-in to devices.
  pub id_consumer_username: Option<String>,
  /// The auth password that will be given on burn-in to devices.
  pub id_consumer_password: Option<String>,

  /// The minimum amount of ids to maintain. If lower than this, we will refill.
  pub registration_pool_minimum: Option<u8>,

  /// The max amount of devices to update during a iteration of checking device activity.
  pub active_device_chunk_size: u8,

  /// Where to send devices on their initial connection
  pub initial_scannable_addr: String,
}

/// The publicly deserializable interface for our registrar worker configuration.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Configuration {
  /// The redis configuration.
  pub redis: crate::config::RedisConfiguration,
  /// The mongo configuration.
  pub mongo: crate::config::MongoConfiguration,
  /// The configuration specific to maintaining a registration of available ids.
  pub registrar: RegistrarConfiguration,
}

impl Configuration {
  /// Builds a worker from whatever we were able to serialize from our configuration inputs.
  pub async fn worker(self) -> io::Result<Worker> {
    let mongo_options = mongodb::options::ClientOptions::parse(&self.mongo.url)
      .await
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    let mongo = mongodb::Client::with_options(mongo_options)
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    Ok(Worker {
      config: self.registrar,
      redis: self.redis,
      connection: None,
      mongo: (mongo, self.mongo.clone()),
    })
  }
}

/// The container that will be passed around to various registrar internal functions.
pub struct Worker {
  /// The redis configuration.
  redis: crate::config::RedisConfiguration,
  /// The TCP connection we have to our redis host, if we currently have one.
  connection: Option<crate::redis::RedisConnection>,
  /// The mongo client + configuration
  mongo: (mongodb::Client, crate::config::MongoConfiguration),
  /// Configuration specific to this worker.
  config: RegistrarConfiguration,
}

impl Worker {
  /// The main execution api of our worker. Inside here we perform the responsibilities of
  /// updating our pool if necessary, and marking whatever devices we've heard from as "active".
  pub async fn work(&mut self) -> io::Result<()> {
    let stream = self.connection.take();

    self.connection = match stream {
      None => {
        log::info!("no previous connection, attempting to connect now");
        crate::redis::connect(&self.redis)
          .await
          .map_err(|error| {
            log::warn!("unable to estabish registrar redis connection - {error}");
            error
          })
          .map(Some)?
      }

      Some(mut redis_connection) => {
        log::trace!("active redis connection, checking pool");

        // Attempt to fill our id pool if necessary.
        let amount = pool::fill_pool(
          &mut redis_connection,
          self.config.registration_pool_minimum.unwrap_or(DEFAULT_POOL_MINIMUM),
        )
        .await?;

        if amount > 0 {
          log::info!("filled pool with '{}' new ids", amount)
        }

        // Attempt to mark all devices that have submitted an incoming ping since our last attempt
        // as active in our diagnostic collection.
        for i in 0..self.config.active_device_chunk_size {
          log::trace!("checking active device queue");
          let amount = diagnostics::mark_active(self, &mut redis_connection).await?;

          if amount == 0 {
            log::info!("no remaining active devices heard from after {i}");
            break;
          }
        }

        if let Err(error) = work_jobs(self, &mut redis_connection).await {
          log::error!("registar job worker failed - {error}");
        }

        Some(redis_connection)
      }
    };

    Ok(())
  }
}

/// Attempts to pop and execute the next job available for us.
async fn work_jobs(worker: &mut Worker, mut redis_connection: &mut crate::redis::RedisConnection) -> io::Result<()> {
  // Attempt to get the next job.
  log::info!("attempting to pop next actual job");
  let next_job = match kramer::execute(
    &mut redis_connection,
    kramer::Command::Lists::<&str, &str>(kramer::ListCommand::Pop(
      kramer::Side::Left,
      crate::constants::REGISTRAR_JOB_QUEUE,
      Some((None, 3)),
    )),
  )
  .await?
  {
    kramer::Response::Array(response_strings) => response_strings
      .get(1)
      .and_then(|kind| match kind {
        kramer::ResponseValue::String(value) => Some(value),
        _ => None,
      })
      .and_then(|string| {
        serde_json::from_str::<RegistrarJob>(string)
          .map_err(|error| {
            log::warn!("failed deserializing registration job - {error}");
            error
          })
          .ok()
      }),
    _ => None,
  };

  if let Some(job_container) = next_job {
    let result = match &job_container.job {
      RegistrarJobKind::Renders(jobs::RegistrarRenderKinds::RegistrationScannable(device_id)) => {
        log::info!("sending initial scannable link to device '{device_id}'");
        let mut initial_url = http_types::Url::parse(&worker.config.initial_scannable_addr).map_err(|error| {
          log::warn!("unable to create initial url for device - {error}");
          io::Error::new(io::ErrorKind::Other, format!("{error}"))
        })?;

        // scope our mutable borrow/mutation so it is dropped before we take ownship when we
        // `to_string` it onto our layout.
        {
          let mut query = initial_url.query_pairs_mut();
          query.append_pair("device_target_id", device_id);
        }

        let mut queue = crate::rendering::queue::Queue::new(redis_connection);
        let layout = crate::rendering::RenderVariant::scannable(initial_url.to_string());
        let job_result = queue
          .queue(&device_id, &crate::rendering::QueuedRenderAuthority::Registrar, layout)
          .await;

        job_result.map(|_| crate::job_result::JobResult::Success)
      }
      RegistrarJobKind::Rename(request) => {
        log::info!("device rename request being processed - {request:?}");
        let job_result = rename::rename(worker, request).await;
        job_result.map(|_| crate::job_result::JobResult::Success)
      }
      RegistrarJobKind::Ownership(o) => {
        log::info!("registrar found next ownership claims job - {o:?}");
        let job_result = ownership::register_device(worker, o).await;
        log::info!("registration result - {job_result:?}");
        job_result.map(|_| crate::job_result::JobResult::Success)
      }
    };

    let serialized_result = match result {
      Ok(c) => serde_json::to_string(&c),
      Err(c) => {
        log::warn!("job failure - {c:?}, recording!");
        serde_json::to_string(&crate::job_result::JobResult::Failure(c.to_string()))
      }
    }
    .map_err(|error| {
      log::warn!("Unable to serialize job result - {error}");
      io::Error::new(io::ErrorKind::Other, format!("job-result-serialization - {error}"))
    })?;
    kramer::execute(
      &mut redis_connection,
      kramer::Command::Hashes(kramer::HashCommand::Set(
        crate::constants::REGISTRAR_JOB_RESULTS,
        kramer::Arity::One((&job_container.id, serialized_result)),
        kramer::Insertion::Always,
      )),
    )
    .await?;
  }

  Ok(())
}
