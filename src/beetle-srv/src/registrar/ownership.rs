use serde::{Deserialize, Serialize};
use std::io;

/// The type job job that will attempt to set the device authority record as owned.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceOwnershipRequest {
  /// The id of the device in question.
  pub(super) device_id: String,

  /// The id of the user in question.
  pub(super) user_id: String,
}

/// Executes the ownership request for the worker. This involves an upsert on our device authority
/// collection, and then checking if the created or existing record allows the user to add the
/// device to their list of available devices.
pub(super) async fn register_device(worker: &mut super::Worker, job: &DeviceOwnershipRequest) -> io::Result<()> {
  let (ref mut mongo, config) = &mut worker.mongo;
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

  if super::user_access(mongo, config, &user.oid, &found_device.id)
    .await?
    .is_none()
  {
    log::warn!("user has no access to the device; rejecting registration");
    return Err(io::Error::new(io::ErrorKind::Other, "no-access"));
  }

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
