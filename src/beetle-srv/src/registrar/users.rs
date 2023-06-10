use std::io;

/// Stores the access token on our user record.
pub async fn process_access_token<S>(
  worker: &mut super::Worker,
  handle: &crate::vendor::google::TokenHandle,
  user_id: S,
) -> io::Result<()>
where
  S: AsRef<str> + std::fmt::Display,
{
  let (mongo_client, mongo_config) = &worker.mongo;

  log::info!(
    "processing new user '{user_id}' access token, created {:?}",
    handle.created
  );

  let collection = mongo_client
    .database(&mongo_config.database)
    .collection::<crate::types::User>(&mongo_config.collections.users);

  let handle_document = bson::to_document(handle).map_err(|error| {
    io::Error::new(
      io::ErrorKind::Other,
      format!("unable to create user update document - {error}"),
    )
  })?;

  let user = collection
    .find_one_and_update(
      bson::doc! { "oid": user_id.as_ref() },
      bson::doc! { "$set": { "latest_token": handle_document } },
      Some(
        mongodb::options::FindOneAndUpdateOptions::builder()
          .return_document(mongodb::options::ReturnDocument::After)
          .build(),
      ),
    )
    .await
    .map_err(|error| {
      log::warn!("unable to update user document - {error}");

      io::Error::new(
        io::ErrorKind::Other,
        format!("unable to update user document - {error}"),
      )
    })?
    .ok_or_else(|| {
      log::warn!("unable to find user for update");
      io::Error::new(io::ErrorKind::Other, "user not found")
    })?;

  log::info!("successfully updated '{}'", user.oid);

  Ok(())
}
