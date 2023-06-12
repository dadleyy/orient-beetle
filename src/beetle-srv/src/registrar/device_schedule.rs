use std::io;

use serde::Deserialize;

/// TODO: this type is a mirror of the schema defined in our `schedule` module, it is likely we can
/// bundle this up in the worker through some api for fetching an access token by user ID.
#[derive(Deserialize, Debug)]
struct UserTokenInfo {
  #[allow(clippy::missing_docs_in_private_items)]
  name: Option<String>,

  #[allow(clippy::missing_docs_in_private_items)]
  latest_token: crate::vendor::google::TokenHandle,
}

/// Attempts to upsert a device schedule and then either replace the found device with a default
/// value or remove that value, depending on its presence.
pub(super) async fn toggle<S>(
  mut worker: super::worker::WorkerHandle<'_>,
  device_id: S,
  user_id: S,
  should_enable: bool,
) -> io::Result<()>
where
  S: AsRef<str>,
{
  let collection = worker
    .mongo
    .client
    .database(&worker.mongo.config.database)
    .collection::<crate::types::DeviceSchedule>(&worker.mongo.config.collections.device_schedules);

  let mut schedule = collection
    .find_one_and_update(
      bson::doc! { "device_id": device_id.as_ref() },
      bson::doc! { "$setOnInsert": { "device_id": device_id.as_ref() } },
      mongodb::options::FindOneAndUpdateOptions::builder()
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .build(),
    )
    .await
    .map_err(|error| {
      log::error!("unable to find/upsert device schedule for '{}'", device_id.as_ref());
      io::Error::new(io::ErrorKind::Other, format!("{error}"))
    })?
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("no schedule found for device '{}'", device_id.as_ref()),
      )
    })?;

  schedule.kind = match (should_enable, schedule.kind.take()) {
    (true, Some(kind)) => Some(kind),
    (true, None) => Some(crate::types::DeviceScheduleKind::UserEventsBasic(
      user_id.as_ref().to_string(),
    )),
    (false, _) => None,
  };

  log::info!("applying new schedule - '{schedule:?}'");

  let result = collection
    .find_one_and_replace(
      bson::doc! { "device_id": device_id.as_ref() },
      &schedule,
      mongodb::options::FindOneAndReplaceOptions::builder()
        .return_document(mongodb::options::ReturnDocument::After)
        .build(),
    )
    .await
    .map_err(|error| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("unable to replace device schedule - {error}"),
      )
    })?;

  worker
    .enqueue_kind(super::RegistrarJobKind::RunDeviceSchedule(
      device_id.as_ref().to_string(),
    ))
    .await?;

  log::info!("schedule update result - '{result:?}'");

  Ok(())
}

/// This method is responsible for immediately running any schedule associated with the device id
/// provded in the job.
pub(super) async fn execute<S>(mut worker: super::worker::WorkerHandle<'_>, device_id: S) -> anyhow::Result<()>
where
  S: AsRef<str>,
{
  let db = worker.mongo.client.database(&worker.mongo.config.database);
  let collection = db.collection::<crate::types::DeviceSchedule>(&worker.mongo.config.collections.device_schedules);

  let schedule = collection
    .find_one(bson::doc! { "device_id": device_id.as_ref() }, None)
    .await
    .map_err(|error| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("unable to query device schedule - {error}"),
      )
    })?
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("unable to find device schedule - '{}'", device_id.as_ref()),
      )
    })?;

  match schedule.kind {
    None => {
      log::info!("nothing to do for device '{}' schedule", device_id.as_ref());
    }
    Some(crate::types::DeviceScheduleKind::UserEventsBasic(user_id)) => {
      log::info!(
        "querying events for device '{}' and user '{}'",
        device_id.as_ref(),
        user_id
      );

      let collection = db.collection::<UserTokenInfo>(&worker.mongo.config.collections.users);

      let mut partial_user = collection
        .find_one(bson::doc! { "oid": &user_id }, None)
        .await
        .map_err(|error| {
          io::Error::new(
            io::ErrorKind::Other,
            format!("unable to query users with access tokens - {error}"),
          )
        })?
        .ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::Other,
            format!("unable to find token for user '{}'", user_id),
          )
        })?;

      // TODO: figure out how to share this decoding logic between here and the `schedule` module
      // which uses it when determining if the access token needs refreshing.
      let key = jsonwebtoken::DecodingKey::from_secret(worker.config.vendor_api_secret.as_bytes());
      let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
      let decoded_token = jsonwebtoken::decode::<super::users::EncodedUserAccessToken>(
        &partial_user.latest_token.token.access_token,
        &key,
        &validation,
      )?;
      partial_user.latest_token.token.access_token = decoded_token.claims.token;

      log::info!(
        "querying calendars for token - '{:?}'",
        partial_user.latest_token.created
      );

      let primary = crate::vendor::google::fetch_primary(&partial_user.latest_token)
        .await?
        .ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::Other,
            format!("no priamry calendar found for user '{user_id}'"),
          )
        })?;

      let events = crate::vendor::google::fetch_events(&partial_user.latest_token, &primary).await?;

      log::info!(
        "found {} events for user '{user_id}' ({:?})",
        events.len(),
        partial_user.name
      );

      if let Some(first_event) = events.get(0) {
        log::info!("rendering event '{first_event:?}'");

        let mut left = vec![crate::rendering::components::StylizedMessage {
          message: first_event.summary.clone(),
          font: crate::rendering::FontSelection::Roboto,
          size: Some(20.0f32),
        }];

        if let Some(start) = first_event.start.date_time.as_ref() {
          left.push(crate::rendering::components::StylizedMessage {
            message: start.clone(),
            font: crate::rendering::FontSelection::Roboto,
            size: Some(10.0f32),
          });
        }

        if let Some(end) = first_event.end.date_time.as_ref() {
          left.push(crate::rendering::components::StylizedMessage {
            message: end.clone(),
            font: crate::rendering::FontSelection::Roboto,
            size: Some(10.0f32),
          });
        }

        let right = crate::rendering::components::StylizedMessage {
          message: partial_user.name.unwrap_or_else(|| "unknown".to_string()),
          font: crate::rendering::FontSelection::Roboto,
          size: Some(20.0f32),
        };

        worker
          .render(
            device_id.as_ref().to_string(),
            crate::rendering::RenderLayout::Split(crate::rendering::SplitLayout {
              left: crate::rendering::SplitContents::Messages(left),
              right: crate::rendering::SplitContents::Messages(vec![right]),
            }),
          )
          .await?;
      }
    }
  }

  Ok(())
}
