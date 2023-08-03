//! This module defines the registrar worker itself. This is the background process that handles
//! pulling jobs off a queue, periodically checking device schedules, and other miscellaneous
//! tasks.
//!
//! General cleanup todo:
//!
//! - Clean up job exectuion; the `work` is basically only responsible for attempting any
//!   necessary reconnects for our redis connection. This could be solved by using some pool-like
//!   structure that will handle that underneath us.
//! - Figure out a better way to perform "scheduled" work; right now that functionality has been
//!   dumped into the `schedule` module adjacent to this.

use super::{device_state, diagnostics, jobs, ownership, pool, rename, users, RegistrarJobKind};
use crate::{config::RegistrarConfiguration, reporting, schema};
use serde::Serialize;
use std::io;

/// If no value is provided in the api, this value will be used as the minimum amount of entries in
/// our pool that we need. If the current amount is less than this, we will generate ids for and
/// store them in the system.
const DEFAULT_POOL_MINIMUM: u8 = 3;

/// The most amount of jobs to try working at once.
const DEFAULT_JOB_BATCH_SIZE: u8 = 50;

/// A wrapping container for our mongo types that provides the api for accessing collection.
pub(super) struct WorkerMongo {
  /// The actual mongodb client.
  pub(super) client: mongodb::Client,
  /// A configuration that holds the names of all our collections.
  pub(super) config: crate::config::MongoConfiguration,
}

impl WorkerMongo {
  /// Constructs our mongo handle. This is async, and should fail fast to provide some early
  /// validation that both the configuration, and url provided are valid.
  pub(super) async fn new<S>(url: S, config: crate::config::MongoConfiguration) -> io::Result<Self>
  where
    S: AsRef<str>,
  {
    let mongo_options = mongodb::options::ClientOptions::parse(&url)
      .await
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    let mongo = mongodb::Client::with_options(mongo_options)
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    Ok(Self { client: mongo, config })
  }

  /// Returns the `mongodb` collection associated with our device schedule schema object.
  pub(super) fn schedules_collection(&self) -> mongodb::Collection<schema::DeviceSchedule> {
    self
      .client
      .database(&self.config.database)
      .collection(&self.config.collections.device_schedules)
  }
}

/// This type provides the api that the worker "hands down" to the various functions it performs
/// during its lifetime.
pub(super) struct WorkerHandle<'a> {
  /// A reference back to the original worker.
  pub(super) mongo: &'a WorkerMongo,
  /// A reference to the workers configuration.
  pub(super) config: &'a RegistrarConfiguration,
  /// A reference to the google configuration.
  pub(super) google: &'a crate::config::GoogleConfiguration,

  /// A reference to the active redis connection. It would be nice if this itself was some
  /// container instead, the way our mongo client is.
  #[allow(unused)]
  redis: &'a mut crate::redis::RedisConnection,
}

impl<'a> WorkerHandle<'a> {
  /// Pushes a render layout onto the queue for a device.
  pub(super) async fn render<I, S>(
    &mut self,
    device_id: I,
    layout: crate::rendering::RenderLayout<S>,
  ) -> io::Result<String>
  where
    I: AsRef<str>,
    S: Serialize,
  {
    let (id, _) = crate::rendering::queue::Queue::new(self.redis, &self.config.vendor_api_secret)
      .queue(
        device_id,
        &crate::rendering::QueuedRenderAuthority::Registrar,
        crate::rendering::RenderVariant::layout(layout),
      )
      .await?;

    Ok(id)
  }

  /// Actually creates the id we will be adding to our queue. This method is preferred to the
  /// `enqueue` method, which expects the consumer to create the job id correctly.
  pub(super) async fn enqueue_kind(&mut self, job: super::RegistrarJobKind) -> io::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let job = super::RegistrarJob { id: id.clone(), job };
    self.enqueue(job).await?;
    Ok(id)
  }

  /// This function can be used by job processing functionality to "percolate" additional jobs
  /// back onto the queue. Such is the case for scheduled access token refreshes.
  async fn enqueue(&mut self, job: super::RegistrarJob) -> io::Result<()> {
    let id = job.id.clone();
    let serialized = job.encrypt(self.config)?;

    let pending_json = serde_json::to_string(&schema::jobs::JobResult::Pending).map_err(|error| {
      log::warn!("unable to serialize pending job state - {error}");
      io::Error::new(io::ErrorKind::Other, "job-serialize")
    })?;

    self
      .command(&kramer::Command::Hashes(kramer::HashCommand::Set(
        crate::constants::REGISTRAR_JOB_RESULTS,
        kramer::Arity::One((&id, &pending_json)),
        kramer::Insertion::Always,
      )))
      .await?;

    self
      .command(&kramer::Command::Lists(kramer::ListCommand::Push(
        (kramer::Side::Right, kramer::Insertion::Always),
        crate::constants::REGISTRAR_JOB_QUEUE,
        kramer::Arity::One(serialized),
      )))
      .await?;

    Ok(())
  }

  /// Returns the mongodb collection for our current device schedules.
  pub fn device_schedule_collection(&mut self) -> io::Result<mongodb::Collection<schema::DeviceSchedule>> {
    Ok(
      self
        .mongo
        .client
        .database(&self.mongo.config.database)
        .collection(&self.mongo.config.collections.device_schedules),
    )
  }

  /// Returns the mongodb collection for our current device state.
  pub fn device_state_collection(&mut self) -> io::Result<mongodb::Collection<schema::DeviceState>> {
    Ok(
      self
        .mongo
        .client
        .database(&self.mongo.config.database)
        .collection(&self.mongo.config.collections.device_states),
    )
  }

  /// The smallest wrapped around kramer redis command execution using our reference to redis.
  async fn command<S, V>(&mut self, command: &kramer::Command<S, V>) -> io::Result<kramer::Response>
  where
    S: std::fmt::Display,
    V: std::fmt::Display,
  {
    kramer::execute(&mut self.redis, command).await
  }
}

/// The container that will be passed around to various registrar internal functions.
pub struct Worker {
  /// The redis configuration.
  pub(super) redis: crate::config::RedisConfiguration,

  /// The TCP connection we have to our redis host, if we currently have one.
  pub(super) connection: Option<crate::redis::RedisConnection>,

  /// The mongo client + configuration
  pub(super) mongo: WorkerMongo,

  /// Configuration specific to this worker.
  pub(super) config: RegistrarConfiguration,

  /// Configuration for google apis.
  pub(super) google: crate::config::GoogleConfiguration,

  /// The handle for our reporting worker.
  pub(super) reporting: Option<async_std::channel::Sender<reporting::Event>>,
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
        let pending_job_count = self.report(&mut redis_connection).await.unwrap_or(1);

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
            log::trace!("no remaining active devices heard from after {i}");
            break;
          }
        }

        if let Err(error) = super::schedule::check_schedule(self.handle(&mut redis_connection)).await {
          log::error!("failed scheduled registrar workflow - {error}");
        }

        let mut processed_job_count = 0u8;
        if pending_job_count > 0 {
          log::trace!("attempting to process {pending_job_count} job(s)");
          while pending_job_count > 0 && processed_job_count < DEFAULT_JOB_BATCH_SIZE {
            log::trace!("attempting to run job attempt #{processed_job_count}");

            processed_job_count += match work_jobs(self, &mut redis_connection).await {
              Err(error) => {
                log::error!("registar job worker failed - {error}");
                break;
              }

              // A none returned if there is nothing left for us to do,
              Ok(None) => break,

              // Otherwise, we still have jobs (this one may have been ignored).
              Ok(Some(amt)) => amt,
            };
          }
        }

        if let Some(sink) = self.reporting.as_ref() {
          if let Err(error) = sink
            .send(reporting::Event::JobBatchProcessed {
              job_count: processed_job_count as u16,
            })
            .await
          {
            log::error!("unable to send job batch processed event - {error}");
          }
        }

        Some(redis_connection)
      }
    };

    Ok(())
  }

  /// Reports queue metrics to the analytics configuration, if any.
  async fn report(&self, redis: &mut crate::redis::RedisConnection) -> Option<u16> {
    let sink = self.reporting.as_ref()?;

    let queue_length = match kramer::execute(
      redis,
      kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Len(crate::constants::REGISTRAR_JOB_QUEUE)),
    )
    .await
    .map_err(|error| {
      log::error!("unable to take length of job queue - {error}");
    })
    .ok()?
    {
      kramer::Response::Item(kramer::ResponseValue::Integer(value)) => value as u16,
      other => {
        log::warn!("strange response from job queue length sample - {other:?}");
        0
      }
    };

    if let Err(error) = sink.send(reporting::Event::JobQueueLengthSample { queue_length }).await {
      log::error!("failed sending event to reporting queue - {error}");
    }

    Some(queue_length)
  }

  /// Internally, this method is used to wrap our valid redis connection with other information
  /// that we will provide to the functions underneath us.
  fn handle<'a>(&'a mut self, redis: &'a mut crate::redis::RedisConnection) -> WorkerHandle<'a> {
    WorkerHandle {
      mongo: &self.mongo,
      config: &self.config,
      google: &self.google,
      redis,
    }
  }
}

/// This is abstracted from below so we can handle the first value in an array response, OR the
/// only value in a string response the same way.
fn decrypt_job<S>(worker: &mut Worker, value: S) -> io::Result<jobs::RegistrarJob>
where
  S: AsRef<str>,
{
  let key = jsonwebtoken::DecodingKey::from_secret(worker.config.vendor_api_secret.as_bytes());
  let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);

  jsonwebtoken::decode::<super::jobs::RegistrarJobEncrypted>(value.as_ref(), &key, &validation)
    .map_err(|error| {
      log::error!("registrar worker unable to decode token - {}", error);
      io::Error::new(io::ErrorKind::Other, "bad-jwt")
    })
    .map(|job_container| job_container.claims.job)
}

/// Attempts to pop and execute the next job available for us. This happens _outside_ our worker's
/// `work` method so we can enforce that we have a valid redis connection to use, which is the
/// primary function of the `work` method.
async fn work_jobs(
  worker: &mut Worker,
  mut redis_connection: &mut crate::redis::RedisConnection,
) -> io::Result<Option<u8>> {
  // Attempt to get the next job.
  log::trace!(
    "attempting to pop next actual job from '{}'",
    crate::constants::REGISTRAR_JOB_QUEUE
  );

  let next_job = match kramer::execute(
    &mut redis_connection,
    kramer::Command::Lists::<&str, &str>(kramer::ListCommand::Pop(
      kramer::Side::Left,
      crate::constants::REGISTRAR_JOB_QUEUE,
      None,
    )),
  )
  .await?
  {
    // If we got a string, attempt to decode it. We're being liberal with our err -> option
    // conversion here; problems along the way can be observed in logs.
    kramer::Response::Array(response_strings) => response_strings
      .get(1)
      .and_then(|kind| match kind {
        kramer::ResponseValue::String(value) => Some(value),
        other => {
          log::error!("strange response from registrar job pop - {other:?}");
          None
        }
      })
      .and_then(|string| {
        log::trace!("pulled encrypted job ({} chars)", string.len());
        decrypt_job(worker, string).ok()
      }),
    kramer::Response::Item(kramer::ResponseValue::String(value)) => decrypt_job(worker, value).ok(),

    // Fortunately, the queue will tell us that we're empty without having to check ourselves.
    kramer::Response::Item(kramer::ResponseValue::Empty) => return Ok(None),

    other => {
      log::error!("very strange response from registrar job pop - {other:?}");
      None
    }
  };

  let job_container = match next_job {
    None => return Ok(Some(0)),
    Some(job) => job,
  };

  log::trace!(
    "jobType[{:?}] is now processing",
    std::mem::discriminant(&job_container.job)
  );

  let mut returned_state = Some(1);

  let result = match &job_container.job {
    RegistrarJobKind::MutateDeviceState(transition) => {
      log::info!(
        "job[{}] processing render state transition for '{}'",
        job_container.id,
        transition.device_id
      );

      device_state::attempt_transition(worker.handle(redis_connection), transition)
        .await
        .map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
    }

    RegistrarJobKind::Renders(super::jobs::RegistrarRenderKinds::CurrentDeviceState(device_id)) => {
      log::debug!(
        "job[{}] processing current device state render request for '{device_id}'",
        job_container.id
      );

      device_state::render_current(worker.handle(redis_connection), device_id)
        .await
        .map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
    }

    RegistrarJobKind::UserAccessTokenRefresh { handle, user_id } => {
      log::info!(
        "job[{}] processing new token refresh for '{}'",
        job_container.id,
        user_id
      );
      users::process_access_token(worker, handle, user_id)
        .await
        .map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }

    // Process requests-for-render-request jobs. This is a bit odd since we already have the
    // renderer jobs too, but is helpful for providing easier ergonomics into sending device
    // registration qr codes.
    RegistrarJobKind::Renders(jobs::RegistrarRenderKinds::RegistrationScannable(device_id)) => {
      log::info!(
        "job[{}] sending initial scannable link to device '{device_id}'",
        job_container.id
      );

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

      let mut queue = crate::rendering::queue::Queue::new(redis_connection, &worker.config.vendor_api_secret);
      let layout = crate::rendering::RenderVariant::scannable(initial_url.to_string());
      let job_result = queue
        .queue(&device_id, &crate::rendering::QueuedRenderAuthority::Registrar, layout)
        .await;

      job_result.map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }

    RegistrarJobKind::RunDeviceSchedule {
      device_id,
      refresh_nonce,
    } => {
      log::info!(
        "job[{}] immediately executing device schedule for '{device_id}'",
        job_container.id
      );

      let execution_result =
        super::device_schedule::execute(worker.handle(redis_connection), &device_id, refresh_nonce.as_ref())
          .await
          .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()));

      // A bit of special casing here - since we have the opportunity to be dealing with stale
      // attempts to run a device schedule, we want to return a `Some(0)` if this job was
      // unprocessable.
      if let Ok(None) = execution_result {
        returned_state = Some(0);
      }

      execution_result.map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }

    RegistrarJobKind::ToggleDefaultSchedule {
      user_id,
      device_id,
      should_enable,
    } => {
      log::info!(
        "job[{}] toggling default device schedule for device '{device_id}' to user '{user_id}' ({should_enable})",
        job_container.id
      );

      super::device_schedule::toggle(worker.handle(redis_connection), device_id, user_id, *should_enable)
        .await
        .map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }

    // Process device rename requests.
    RegistrarJobKind::Rename(request) => {
      log::info!("device rename request being processed - {request:?}");
      let job_result = rename::rename(worker, request).await;
      job_result.map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }

    // Process device ownership change requests.
    RegistrarJobKind::OwnershipChange(request) => {
      let job_result = ownership::process_change(worker, request).await;
      job_result.map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }

    // Process device ownership claiming requests.
    RegistrarJobKind::Ownership(ownership_request) => {
      log::debug!("registrar found next ownership claims job - '{ownership_request:?}'");
      let job_result = ownership::register_device(worker, ownership_request).await;
      log::debug!("registration result - {job_result:?}");
      job_result.map(|_| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal))
    }
  };

  let serialized_result = match result {
    Ok(job_result) => serde_json::to_string(&job_result),
    Err(job_error) => {
      log::error!("job failure - {job_error:?}, recording!");
      serde_json::to_string(&schema::jobs::JobResult::Failure(job_error.to_string()))
    }
  }
  .map_err(|error| {
    log::error!("Unable to serialize job result - {error}");
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

  Ok(returned_state)
}
