use serde::{Deserialize, Serialize};
use std::io;

/// A request to rename a device.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceRenameRequest {
  /// the device.
  pub device_id: String,
  /// the name.
  pub new_name: String,
}

/// This method processes a device rename request by updating the device diagnostic itself, as well
/// as the device maps for all users that have added the device.
pub async fn rename(worker: &mut super::Worker, request: &DeviceRenameRequest) -> io::Result<()> {
  let (ref mut mongo, config) = &mut worker.mongo;

  let device_collection = mongo
    .database(&config.database)
    .collection(&config.collections.device_diagnostics);

  let users_collection = mongo
    .database(&config.database)
    .collection::<crate::types::User>(&config.collections.users);

  log::info!("attepting to find device for rename request '{request:?}'");

  let found_device: crate::types::DeviceDiagnostic = device_collection
    .find_one_and_update(
      bson::doc! { "id": &request.device_id },
      bson::doc! { "$set": { "nickname": &request.new_name } },
      Some(
        mongodb::options::FindOneAndUpdateOptions::builder()
          .return_document(mongodb::options::ReturnDocument::After)
          .build(),
      ),
    )
    .await
    .map_err(|error| {
      log::warn!("unable to find device - {error}");
      io::Error::new(io::ErrorKind::Other, "failed-update")
    })?
    .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "device not found"))?;

  let device_key = format!("devices.{}", &request.device_id);
  let snapshot = found_device.snapshot();
  let snapshot_doc =
    bson::to_document(&snapshot).map_err(|error| io::Error::new(io::ErrorKind::Other, format!("{error}")))?;
  log::info!("using device key '{device_key}' for update key on users");

  let update_result = users_collection
    .update_many(
      bson::doc! { "devices": { "$exists": 1 } },
      bson::doc! { "$set": { device_key: snapshot_doc } },
      None,
    )
    .await;

  match update_result {
    Err(error) => {
      log::warn!("unable to update user records for '{found_device:?}' - {error:?}");
    }
    Ok(update_info) => {
      log::info!("updated user records with new snapshot - '{update_info:?}'");
    }
  }

  log::info!("found device for rename - '{found_device:?}'");

  Ok(())
}
