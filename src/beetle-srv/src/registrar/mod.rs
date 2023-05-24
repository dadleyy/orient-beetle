use serde::{Deserialize, Serialize};
use std::io;

/// The ownership model defines types + functions associated with managing a devices ownership.
pub mod ownership;

/// This module defines functionality associated with managing the acl pool.
mod pool;

/// This module defines functionality associated with managing the acl pool.
mod diagnostics;

/// Just a place to put the types generally associated with background work.
pub mod jobs;
pub use jobs::{DeviceRenameRequest, RegistrarJob, RegistrarJobKind};

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

      Some(mut inner) => {
        log::trace!("active redis connection, checking pool");

        // Attempt to fill our id pool if necessary.
        let amount = pool::fill_pool(
          &mut inner,
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
          let amount = diagnostics::mark_active(self, &mut inner).await?;

          if amount == 0 {
            log::info!("no remaining active devices heard from after {i}");
            break;
          }
        }

        if let Err(error) = work_jobs(self, &mut inner).await {
          log::error!("registar job worker failed - {error}");
        }

        Some(inner)
      }
    };

    Ok(())
  }
}

/// Attempts to pop and execute the next job available for us.
async fn work_jobs(worker: &mut Worker, mut inner: &mut crate::redis::RedisConnection) -> io::Result<()> {
  // Attempt to get the next job.
  log::info!("attempting to pop next actual job");
  let next_job = match kramer::execute(
    &mut inner,
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
      RegistrarJobKind::Rename(request) => {
        log::info!("device rename request being processed - {request:?}");
        Ok(crate::job_result::JobResult::Success)
      }
      RegistrarJobKind::Ownership(o) => {
        log::info!("registrar found next ownership claims job - {o:?}");
        let job_result = register_device(worker, o).await;
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
      &mut inner,
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

/// The access level a user has to a given device.
#[derive(Serialize, Debug)]
pub enum AccessLevel {
  /// The user can do anything.
  All,
}

/// Returns the access level that a given user has for a given device.
pub async fn user_access(
  mongo: &mongodb::Client,
  config: &crate::config::MongoConfiguration,
  user_id: &String,
  device_id: &String,
) -> io::Result<Option<AccessLevel>> {
  let authority_collection = mongo
    .database(&config.database)
    .collection(&config.collections.device_authorities);

  // Now we want to find the authority record associated with this device. If there isn't one
  // already, one will be created with a default, exclusing model for the current user.
  let initial_auth = Some(crate::types::DeviceAuthorityModel::Exclusive(user_id.clone()));
  let serialized_auth = bson::to_bson(&initial_auth).map_err(|error| {
    log::warn!("unable to prepare initial auth - {error}");
    io::Error::new(io::ErrorKind::Other, "authority-serialization")
  })?;

  let authority_record: Option<crate::types::DeviceAuthorityRecord> = authority_collection
    .find_one_and_update(
      bson::doc! { "device_id": &device_id },
      bson::doc! { "$setOnInsert": { "authority_model": serialized_auth } },
      Some(
        mongodb::options::FindOneAndUpdateOptions::builder()
          .upsert(true)
          .return_document(mongodb::options::ReturnDocument::After)
          .build(),
      ),
    )
    .await
    .map_err(|error| {
      log::warn!("unable to find authority record for device - {error}");
      io::Error::new(io::ErrorKind::Other, "failed-update")
    })?;

  // With the preexisting model, or our newly created, exclusive one, just the verification as user
  // against the current user.
  log::debug!("current authority record - {authority_record:?}");

  match authority_record.as_ref().and_then(|rec| rec.authority_model.as_ref()) {
    Some(crate::types::DeviceAuthorityModel::Shared(owner, guests)) => {
      let mut found = false;
      for guest in guests {
        if guest == user_id {
          found = true;
          break;
        }
      }

      if owner != user_id && !found {
        return Ok(None);
      }
    }
    Some(crate::types::DeviceAuthorityModel::Exclusive(owner)) => {
      if owner != user_id {
        return Ok(None);
      }
    }
    other => {
      log::info!("authority model '{other:?}' checks out, adding '{}'", user_id);
    }
  }

  Ok(Some(AccessLevel::All))
}

/// Executes the ownership request for the worker. This involves an upsert on our device authority
/// collection, and then checking if the created or existing record allows the user to add the
/// device to their list of available devices.
async fn register_device(worker: &mut Worker, job: &ownership::DeviceOwnershipRequest) -> io::Result<()> {
  let (ref mut mongo, config) = &mut worker.mongo;

  let device_collection = mongo
    .database(&config.database)
    .collection(&config.collections.device_diagnostics);

  let users = mongo.database(&config.database).collection(&config.collections.users);

  // Find the user requesting this device.
  let mut user: crate::types::User = users
    .find_one(bson::doc! { "oid": &job.user_id }, None)
    .await
    .map_err(|error| {
      log::warn!("unable to find device - {error}");
      io::Error::new(io::ErrorKind::Other, "failed-update")
    })?
    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "device not found"))?;

  // Find the device for this request. By now, the device should've sent _at least_ one ping to the
  // server after receiving its identifier.
  let found_device: crate::types::DeviceDiagnostic = device_collection
    .find_one(bson::doc! { "id": &job.device_id }, None)
    .await
    .map_err(|error| {
      log::warn!("unable to find device - {error}");
      io::Error::new(io::ErrorKind::Other, "failed-update")
    })?
    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "device not found"))?;

  if user_access(mongo, config, &user.oid, &found_device.id).await?.is_none() {
    log::warn!("user has no access to the device; rejecting registration");
    return Err(io::Error::new(io::ErrorKind::Other, "no-access"));
  }

  let query = bson::doc! { "oid": job.user_id.clone() };

  // Update or create our new devices hash for this user.
  let devices = user
    .devices
    .take()
    .map(|mut existing_devices| {
      existing_devices.insert(job.device_id.clone(), 1);
      existing_devices
    })
    .or_else(|| {
      let mut start = std::collections::HashMap::with_capacity(1);
      start.insert(job.device_id.clone(), 0);
      Some(start)
    });

  let updated = crate::types::User { devices, ..user };
  let options = mongodb::options::FindOneAndUpdateOptions::builder()
    .upsert(true)
    .return_document(mongodb::options::ReturnDocument::After)
    .build();

  users
    .find_one_and_update(
      query,
      bson::doc! { "$set": bson::to_bson(&updated).map_err(|error| {
        log::warn!("unable to serialize user update - {error}");
        io::Error::new(io::ErrorKind::Other, "bad-serialize")
      })? },
      options,
    )
    .await
    .map_err(|error| {
      log::warn!("unable to create new user - {:?}", error);
      io::Error::new(io::ErrorKind::Other, "failed-update")
    })?;

  // Wrap up by updating the diagnostic itself so we can keep track of the original owner.
  let updated_reg = crate::types::DeviceDiagnosticRegistration::Owned(crate::types::DeviceDiagnosticOwnership {
    original_owner: job.user_id.clone(),
  });
  let serialized_registration = bson::to_bson(&updated_reg).map_err(|error| {
    log::warn!("unable to serialize registration_state: {error}");
    io::Error::new(io::ErrorKind::Other, format!("{error}"))
  })?;

  if let Err(error) = device_collection
    .find_one_and_update(
      bson::doc! { "id": found_device.id },
      bson::doc! { "$set": { "registration_state": serialized_registration } },
      mongodb::options::FindOneAndUpdateOptions::builder()
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .build(),
    )
    .await
  {
    log::warn!("unable to update device registration state - {error}");
  }

  Ok(())
}
