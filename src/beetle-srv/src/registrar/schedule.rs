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
  #[allow(clippy::missing_docs_in_private_items)]
  latest_token: crate::vendor::google::TokenHandle,
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

    let now = chrono::Utc::now();
    let diff = now
      .signed_duration_since(current_handle.latest_token.created)
      .num_seconds()
      .abs_diff(0);

    let expiration_diff = current_handle
      .latest_token
      .token
      .expires_in
      .checked_sub(diff)
      .unwrap_or_default();

    log::debug!(
      "next user access token - '{}' (created {diff} seconds ago) (expires in {expiration_diff} seconds)",
      current_handle.oid
    );

    if expiration_diff < EXPIRATION_BUFFER {
      let key = jsonwebtoken::DecodingKey::from_secret(worker.config.vendor_api_secret.as_bytes());
      let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
      let mut replaced_tokens = false;

      match jsonwebtoken::decode::<EncodedUserAccessToken>(
        &current_handle.latest_token.token.access_token,
        &key,
        &validation,
      ) {
        Err(error) => {
          log::warn!("unable to decode acccess token - {error}, scheduling cleanup");
          expired_user_ids.push(current_handle.oid);
          continue;
        }
        Ok(current_token) => {
          log::trace!("decoded original access token - '{}'", current_token.claims.token);
          current_handle.latest_token.token.access_token = current_token.claims.token;
        }
      }

      if let Some(refresh) = &current_handle
        .latest_token
        .token
        .refresh_token
        .as_ref()
        .and_then(|refresh| {
          jsonwebtoken::decode::<EncodedUserAccessToken>(refresh.as_str(), &key, &validation)
            .map_err(|error| {
              log::warn!("unable to decode peristed access token - {error}");
            })
            .ok()
        })
      {
        log::trace!("decoded refresh token - '{:?}'", refresh.claims);
        current_handle.latest_token.token.refresh_token = Some(refresh.claims.token.clone());
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
      match refresh_token(worker.google, &current_handle.latest_token).await {
        Err(error) => {
          log::warn!("unable to refresh token for user '{}' ({error})", current_handle.oid);
        }
        Ok(mut updated_token) => {
          log::info!(
            "successfully updated token, queuing job to persist '{:?}'",
            updated_token.created
          );

          // Be sure to persist the refresh token itself across updates.
          updated_token.token.refresh_token = current_handle.latest_token.token.refresh_token;
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
  log::info!("registrar now checking for any schedules due for a refresh");
  let schedules_collection = worker.device_schedule_collection()?;
  let cutoff = chrono::Utc::now()
    .checked_sub_signed(chrono::Duration::seconds(SCHEDULE_REFRESH_SECONDS))
    .ok_or_else(|| anyhow::Error::msg("unable to create cutoff date for device schedule refresh"))?
    .timestamp_millis();

  let mut cursor = schedules_collection
    .find(
      bson::doc! { "last_executed": { "$lt": cutoff } },
      mongodb::options::FindOptions::builder().limit(10).build(),
    )
    .await?;

  log::debug!("queried device schedules with cutoff");

  while let Some(handle_result) = async_std::stream::StreamExt::next(&mut cursor).await {
    log::debug!("found schedule needing refresh - {handle_result:?}");

    let device_id = match handle_result {
      Err(error) => {
        log::warn!("strange device schedule problem - {error}");
        continue;
      }
      Ok(schedule) => schedule.device_id,
    };

    if let Err(error) = worker
      .enqueue_kind(super::RegistrarJobKind::RunDeviceSchedule(device_id))
      .await
    {
      log::error!("unable to queue device schedule execution job - {error}");
    }
  }

  Ok(())
}

/// Queries the user collection, gets refresh tokens.
pub(super) async fn check_schedule(mut worker: super::worker::WorkerHandle<'_>) -> anyhow::Result<()> {
  check_tokens(&mut worker).await?;
  check_schedules(&mut worker).await
}
