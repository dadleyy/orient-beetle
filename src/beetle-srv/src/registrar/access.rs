use serde::Serialize;
use std::io;

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
  log::trace!("current authority record - {authority_record:?}");

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
