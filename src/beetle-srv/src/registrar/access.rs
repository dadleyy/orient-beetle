use crate::schema;
use serde::Serialize;
use std::io;

/// The access level a user has to a given device.
#[derive(Serialize, Debug)]
pub enum AccessLevel {
  /// The user can do anything.
  All,
}

/// Returns the access level that a given user has for a given device, as well as the record that
/// was found. The latter is useful in situations where we will be updating it after checking the
/// permissions.
pub async fn user_access(
  mongo: &mongodb::Client,
  config: &crate::config::MongoConfiguration,
  user_id: &String,
  device_id: &String,
) -> io::Result<Option<(AccessLevel, Option<schema::DeviceAuthorityRecord>)>> {
  let authority_collection = mongo
    .database(&config.database)
    .collection(&config.collections.device_authorities);

  // Now we want to find the authority record associated with this device. If there isn't one
  // already, one will be created with a default, exclusing model for the current user.
  let initial_auth = Some(schema::DeviceAuthorityModel::Exclusive { owner: user_id.clone() });

  let serialized_auth = bson::to_bson(&initial_auth).map_err(|error| {
    log::warn!("unable to prepare initial auth - {error}");
    io::Error::new(io::ErrorKind::Other, "authority-serialization")
  })?;

  let authority_record: Option<schema::DeviceAuthorityRecord> = authority_collection
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
    Some(schema::DeviceAuthorityModel::Shared { owner, guests }) => {
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
    Some(schema::DeviceAuthorityModel::Exclusive { owner }) => {
      if owner != user_id {
        return Ok(None);
      }
    }
    Some(schema::DeviceAuthorityModel::Public { .. }) => return Ok(Some((AccessLevel::All, authority_record))),
    None => {
      log::warn!("no authority record found for '{device_id}'!");
    }
  }

  Ok(Some((AccessLevel::All, authority_record)))
}
