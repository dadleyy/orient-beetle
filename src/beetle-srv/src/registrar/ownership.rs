use serde::{Deserialize, Serialize};
use std::io;

/// The type jobs dealing with changing the authority record model.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceOwnershipChangeRequest {
  /// Will attempt to toggle the ownership model between public and private. When the value is
  /// truthy, we will be public.
  SetPublicAvailability(String, bool),
}

/// The typejob that will attempt to set the device authority record as owned.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceOwnershipRequest {
  /// The id of the device in question.
  pub(super) device_id: String,

  /// The id of the user in question.
  pub(super) user_id: String,
}

/// This is the real worker for this "ownership" kind of job.
pub(super) async fn process_change(worker: &mut super::Worker, job: &DeviceOwnershipChangeRequest) -> io::Result<()> {
  let super::worker::WorkerMongo { client: mongo, config } = &worker.mongo;

  let models = mongo
    .database(&config.database)
    .collection::<crate::types::DeviceAuthorityRecord>(&config.collections.device_authorities);

  match job {
    DeviceOwnershipChangeRequest::SetPublicAvailability(id, state) => {
      let model = models
        .find_one(bson::doc! { "device_id": &id }, None)
        .await
        .map_err(|error| {
          io::Error::new(
            io::ErrorKind::Other,
            format!("failed finding '{id}' ownership record - {error}"),
          )
        })?
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "not-found"))?;

      let new_state = match (&model.authority_model, state) {
        (Some(crate::types::DeviceAuthorityModel::Exclusive(owner)), true) => {
          log::info!("moving from private, exclusive to public");
          crate::types::DeviceAuthorityModel::Public(owner.clone(), vec![])
        }
        (Some(crate::types::DeviceAuthorityModel::Public(owner, members)), false) => {
          log::info!("moving from public to shared");
          crate::types::DeviceAuthorityModel::Shared(owner.clone(), members.clone())
        }
        other => {
          log::warn!("toggling public availability means nothing in combination with '{other:?}'");
          return Ok(());
        }
      };

      log::info!("will attempt to toggle ownership record - '{model:?}' -> '{new_state:?}'");
      let new_model = crate::types::DeviceAuthorityRecord {
        device_id: id.clone(),
        authority_model: Some(new_state),
      };
      // let updates = bson::to_document(&new_model).map_err(|error| {
      //   log::warn!("unable to serialize - {error}");
      //   io::Error::new(io::ErrorKind::Other, "serialization failure (auth model)")
      // })?;
      let result = models
        .find_one_and_replace(
          bson::doc! { "device_id": &id },
          new_model,
          Some(
            mongodb::options::FindOneAndReplaceOptions::builder()
              .return_document(mongodb::options::ReturnDocument::After)
              .build(),
          ),
        )
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to update model - {error}")))?;
      log::info!("matched update count - '{:?}'", result);
    }
  }

  Ok(())
}

/// Executes the ownership request for the worker. This involves an upsert on our device authority
/// collection, and then checking if the created or existing record allows the user to add the
/// device to their list of available devices.
pub(super) async fn register_device(worker: &mut super::Worker, job: &DeviceOwnershipRequest) -> io::Result<()> {
  let super::worker::WorkerMongo { client: mongo, config } = &worker.mongo;
  log::info!("processing device registration for '{job:?}'");

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

  // Check to see if the user can access this device.
  let (_, authority_model) = super::user_access(mongo, config, &user.oid, &found_device.id)
    .await?
    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no-access"))?;

  log::info!("user available for access, updating authority model with our id ({authority_model:?})");

  let query = bson::doc! { "oid": job.user_id.clone() };

  // Update or create our new devices hash for this user.
  let devices = user
    .devices
    .take()
    .map(|mut existing_devices| {
      existing_devices.insert(job.device_id.clone(), found_device.snapshot());
      existing_devices
    })
    .or_else(|| {
      let mut start = std::collections::HashMap::with_capacity(1);
      start.insert(job.device_id.clone(), found_device.snapshot());
      Some(start)
    });

  log::info!("new device map for user '{}' - '{devices:?}'", job.user_id);

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

  if let Some(crate::types::DeviceAuthorityRecord {
    device_id,
    authority_model: Some(crate::types::DeviceAuthorityModel::Public(original, mut public_users)),
  }) = authority_model
  {
    log::info!("adding user to the public authority model tracking for device '{device_id}'");
    public_users.push(job.user_id.clone());
    let new_model = crate::types::DeviceAuthorityRecord {
      device_id: device_id.clone(),
      authority_model: Some(crate::types::DeviceAuthorityModel::Public(original, public_users)),
    };

    let models = mongo
      .database(&config.database)
      .collection::<crate::types::DeviceAuthorityRecord>(&config.collections.device_authorities);

    let updates = bson::to_document(&new_model).map_err(|error| {
      log::warn!("unable to serialize - {error}");
      io::Error::new(io::ErrorKind::Other, "serialization failure (auth model)")
    })?;

    if let Err(error) = models
      .find_one_and_update(bson::doc! { "device_id": &device_id }, updates, None)
      .await
    {
      log::warn!("unable to update authority model with new user - {error}");
    }
  }

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
