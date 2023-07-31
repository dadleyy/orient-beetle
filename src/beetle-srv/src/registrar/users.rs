use crate::schema;
use serde::{Deserialize, Serialize};
use std::io;

/// This is the schema of the encoded string that will be persisted onto user records. These are
/// effectively the JSON web token claims.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct EncodedUserAccessToken {
  /// The time this token expires.
  pub exp: usize,
  /// The original token value.
  pub(super) token: String,
}

/// This job is responsible for taking an access token that was created during some OAuth token
/// exchange and peristing it back onto the user record. During this process, the token is wrapped
/// in a serializable struct and turned into a jwt.
pub async fn process_access_token<S>(
  worker: &mut super::Worker,
  handle: &crate::vendor::google::TokenHandle,
  user_id: S,
) -> io::Result<()>
where
  S: AsRef<str> + std::fmt::Display,
{
  let super::worker::WorkerMongo {
    client: mongo_client,
    config: mongo_config,
  } = &worker.mongo;

  // Get a timestamp that we will use as the expiration of our tokens.
  let day = chrono::Utc::now()
    .checked_add_signed(chrono::Duration::hours(24))
    .unwrap_or_else(chrono::Utc::now);

  let exp = day.timestamp() as usize;

  log::info!(
    "processing new user '{user_id}' access token, created {:?}",
    handle.created
  );

  let collection = mongo_client
    .database(&mongo_config.database)
    .collection::<schema::User>(&mongo_config.collections.users);

  let header = &jsonwebtoken::Header::default();
  let secret = jsonwebtoken::EncodingKey::from_secret(worker.config.vendor_api_secret.as_bytes());
  let encoded_token = jsonwebtoken::encode(
    header,
    &EncodedUserAccessToken {
      token: handle.token.access_token.clone(),
      exp,
    },
    &secret,
  )
  .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to encode access token - {error}")))?;

  // Clone and mutate our original handle on the way out.
  let mut handle_copy = handle.clone();
  handle_copy.token.access_token = encoded_token;

  // If there is a refresh token too, encrypt that as well.
  handle_copy.token.refresh_token = handle_copy.token.refresh_token.and_then(|refresh_token| {
    jsonwebtoken::encode(
      header,
      &EncodedUserAccessToken {
        token: refresh_token,
        exp,
      },
      &secret,
    )
    .map_err(|error| {
      log::warn!("unable to encrypt refresh token - {error}; this user ('{user_id}') will not have one peristed");
      error
    })
    .ok()
  });

  let handle_document = bson::to_document(&handle_copy).map_err(|error| {
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

  log::debug!("successfully updated '{}'", user.oid);

  Ok(())
}
