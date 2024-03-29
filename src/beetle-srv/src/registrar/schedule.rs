use anyhow::Context;
use serde::Deserialize;
use std::io;

use super::users::EncodedUserAccessToken;

/// The amount of time to buffer between a token expriting and we refresh it.
const EXPIRATION_BUFFER: u64 = 1000;

/// The amount of seconds between device schedule refreshes.
const SCHEDULE_REFRESH_SECONDS: i64 = 60 * 5;

/// This type represents the partial schema from our users collection that we are concerned with
/// here.
#[derive(Deserialize, Debug)]
struct UserTokenInfo {
  #[allow(clippy::missing_docs_in_private_items)]
  oid: String,

  /// The latest refresh + access tokens embedded in the user document.
  latest_token: Option<crate::vendor::google::TokenHandle>,
}

/// Performs a token refresh.
async fn refresh_token(
  config: &crate::config::GoogleConfiguration,
  handle: &crate::vendor::google::TokenHandle,
) -> anyhow::Result<crate::vendor::google::TokenHandle> {
  let refresh_token = handle
    .token
    .refresh_token
    .as_ref()
    .ok_or_else(|| anyhow::Error::msg("no refresh token available on {handle:?}"))?;

  log::trace!("refreshing token for handle {refresh_token:?}");

  let mut response = surf::post("https://oauth2.googleapis.com/token")
    .body_json(&crate::vendor::google::TokenRefreshRequest {
      refresh_token: refresh_token.clone(),
      client_id: config.client_id.clone(),
      client_secret: config.client_secret.clone(),
      grant_type: "refresh_token",
    })
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
    .with_context(|| "failed refresh body serialization")?
    .await
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
    .with_context(|| "failed sending refresh")?;

  let body_string = response
    .body_string()
    .await
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
    .with_context(|| "failed reading refresh body")?;

  log::trace!("refresh body - '{body_string}'");

  let parsed = serde_json::from_str::<crate::vendor::google::TokenResponse>(&body_string)?;

  Ok(crate::vendor::google::TokenHandle {
    created: chrono::Utc::now(),
    token: parsed,
  })
}

/// This background method will attempt to find close-to-expiring oauth access tokens and request
/// new ones from the oauth providers.
async fn check_tokens(worker: &mut super::worker::WorkerHandle<'_>) -> io::Result<()> {
  let super::worker::WorkerMongo {
    client: mongo,
    config: mongo_config,
  } = &worker.mongo;

  let collection = mongo
    .database(&mongo_config.database)
    .collection::<UserTokenInfo>(&mongo_config.collections.users);

  let mut cursor = collection
    .find(
      bson::doc! { "latest_token": { "$exists": 1 } },
      mongodb::options::FindOptions::builder().limit(10).build(),
    )
    .await
    .map_err(|error| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("unable to query users with access tokens - {error}"),
      )
    })?;

  let mut expired_user_ids = vec![];

  while let Some(handle_result) = async_std::stream::StreamExt::next(&mut cursor).await {
    let mut current_handle = match handle_result {
      Err(error) => {
        log::warn!("unable to deserialize next scheduled user access token refresh - {error}");
        break;
      }
      Ok(c) => c,
    };

    let token_ref = match current_handle.latest_token.as_mut() {
      None => {
        log::warn!("user '{}' is missing a 'latest_token'", current_handle.oid);
        continue;
      }
      Some(token) => token,
    };

    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(token_ref.created).num_seconds().abs_diff(0);
    let expiration_diff = token_ref.token.expires_in.checked_sub(diff).unwrap_or_default();

    log::trace!(
      "next user access token - '{}' (created {diff} seconds ago) (expires in {expiration_diff} seconds)",
      current_handle.oid
    );

    if expiration_diff < EXPIRATION_BUFFER {
      let key = jsonwebtoken::DecodingKey::from_secret(worker.config.vendor_api_secret.as_bytes());
      let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
      let mut replaced_tokens = false;

      match jsonwebtoken::decode::<EncodedUserAccessToken>(&token_ref.token.access_token, &key, &validation) {
        Err(error) => {
          log::warn!("unable to decode acccess token - {error}, scheduling cleanup");
          expired_user_ids.push(current_handle.oid);
          continue;
        }
        Ok(current_token) => {
          log::trace!("decoded original access token - '{}'", current_token.claims.token);
          token_ref.token.access_token = current_token.claims.token;
        }
      }

      if let Some(refresh) = token_ref.token.refresh_token.as_ref().and_then(|refresh| {
        jsonwebtoken::decode::<EncodedUserAccessToken>(refresh.as_str(), &key, &validation)
          .map_err(|error| {
            log::warn!("unable to decode peristed access token - {error}");
          })
          .ok()
      }) {
        log::trace!("decoded refresh token - '{:?}'", refresh.claims);
        token_ref.token.refresh_token = Some(refresh.claims.token.clone());
        replaced_tokens = true;
      }

      if !replaced_tokens {
        log::warn!(
          "ignoring potentially expired token for user '{}', was unable to validate refresh token",
          current_handle.oid
        );
        continue;
      }

      // Now we can actually attempt to make our api request for a new token. If it succeeds, we
      // will enqueue a job to persist it onto the user document, which will take care of
      // performing the encryption for us.
      match refresh_token(worker.google, token_ref).await {
        Err(error) => {
          log::warn!("unable to refresh token for user '{}' ({error})", current_handle.oid);
        }
        Ok(mut updated_token) => {
          log::trace!(
            "successfully updated token, queuing job to persist '{:?}'",
            updated_token.created
          );

          // Be sure to persist the refresh token itself across updates.
          updated_token.token.refresh_token = token_ref.token.refresh_token.clone();
          let job = super::RegistrarJobKind::UserAccessTokenRefresh {
            handle: updated_token,
            user_id: current_handle.oid,
          };

          if let Err(error) = worker.enqueue_kind(job).await {
            log::warn!("failed access token refresh percolation - {error}");
          }
        }
      }
    }
  }

  // If any tokens were unable to be decoded, update the user records, removing them. This will
  // race against any users that are currently logging in, assuming there are multiple registrar
  // workers running.
  if !expired_user_ids.is_empty() {
    log::warn!("cleaning up {} user access tokens", expired_user_ids.len());
    if let Err(error) = collection
      .update_many(
        bson::doc! { "oid": { "$in": expired_user_ids } },
        bson::doc! { "$unset": { "latest_token": "" } },
        None,
      )
      .await
    {
      log::error!("unable to cleanup failed tokens - '{error}'");
    }
  }

  Ok(())
}

/// This is the background method responsible for querying the device schedules collection for any
/// that have not been run in some time. For these, the worker will queue an execution job and move
/// onto the next one.
async fn check_schedules(worker: &mut super::worker::WorkerHandle<'_>) -> anyhow::Result<()> {
  log::trace!("registrar now checking for any schedules due for a refresh");

  let schedules_collection = worker.device_schedule_collection()?;
  let interval_seconds = worker
    .config
    .device_schedule_refresh_interval_seconds
    .as_ref()
    .copied()
    .unwrap_or(SCHEDULE_REFRESH_SECONDS);

  let cutoff = chrono::Utc::now()
    .checked_sub_signed(chrono::Duration::seconds(interval_seconds))
    .ok_or_else(|| anyhow::Error::msg("unable to create cutoff date for device schedule refresh"))?
    .timestamp_millis();

  let mut cursor = schedules_collection
    .find(
      bson::doc! { "last_executed": { "$lt": cutoff } },
      mongodb::options::FindOptions::builder().limit(10).build(),
    )
    .await?;

  log::trace!("queried device schedules with cutoff - {cutoff}");
  let mut nonce_updates = vec![];

  while let Some(handle_result) = async_std::stream::StreamExt::next(&mut cursor).await {
    log::trace!("found schedule needing refresh - {handle_result:?}");

    let schedule = match handle_result {
      Err(error) => {
        log::error!("strange device schedule problem - {error}");
        continue;
      }
      Ok(schedule) => schedule,
    };

    let device_id = schedule.device_id;

    match (schedule.refresh_nonce, schedule.latest_refresh_nonce) {
      (Some(current), Some(latest)) if current == latest => {}
      (None, None) => {
        log::warn!("schedule['{device_id}'] no previous nonce, setting now");
      }
      _ => {
        log::trace!("schedule['{device_id}'] will be handled by next execution job");
        continue;
      }
    }

    let new_nonce = uuid::Uuid::new_v4().to_string();
    log::info!("schedule['{device_id}'] device is ready for refresh -> {new_nonce}");
    nonce_updates.push((device_id.clone(), new_nonce.clone()));

    if let Err(error) = worker
      .enqueue_kind(super::RegistrarJobKind::RunDeviceSchedule {
        device_id,
        refresh_nonce: Some(new_nonce),
      })
      .await
    {
      log::error!("unable to queue device schedule execution job - {error}");
    }
  }

  let schedules = worker.mongo.schedules_collection();
  for (device_id, new_nonce) in nonce_updates {
    let result = schedules
      .find_one_and_update(
        bson::doc! { "device_id": &device_id },
        bson::doc! { "$set": { "refresh_nonce": new_nonce } },
        mongodb::options::FindOneAndUpdateOptions::builder()
          .return_document(mongodb::options::ReturnDocument::After)
          .build(),
      )
      .await;

    match result {
      Err(error) => {
        log::error!("unable to update device '{device_id}' nonce - {error}");
      }
      Ok(None) => log::error!("unable to find device '{device_id}'"),
      Ok(Some(_)) => log::trace!("successfully updated device '{device_id}'"),
    }
  }

  Ok(())
}

/// Queries the user collection, gets refresh tokens.
pub(super) async fn check_schedule(mut worker: super::worker::WorkerHandle<'_>) -> anyhow::Result<()> {
  check_tokens(&mut worker).await?;
  check_schedules(&mut worker).await
}
