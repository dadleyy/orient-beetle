use serde::{Deserialize, Serialize};

/// The payload for looking up a device by id.
#[derive(Debug, Deserialize)]
struct LookupQuery {
  /// The id of a device in question.
  id: String,
}

/// The schema of our api to registration of a device.
#[derive(Debug, Deserialize)]
struct RegistrationPayload {
  /// The id of the device.
  device_id: String,
}

/// The schema of responses sent from the registration api.
#[derive(Debug, Serialize)]
struct RegistrationResponse {
  /// The id of the device registered.
  id: String,
}

/// The schema of responses sent from the device api.
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct DeviceInfoPayload {
  /// The device id.
  id: String,

  /// The timestamp of the first occurance of a pop from our device.
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  first_seen: Option<chrono::DateTime<chrono::Utc>>,

  /// The timestamp of the last occurance of a pop from our device.
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  last_seen: Option<chrono::DateTime<chrono::Utc>>,

  /// The amount of messages sent to this device. A `None` represents some unknown state.
  sent_message_count: Option<u32>,

  /// How many messages are currently pending.
  current_queue_count: i64,

  /// The nickname set (if any).
  nickname: Option<String>,

  /// A list of the most recent messages that have been sent to the device.
  sent_messages: Vec<crate::rendering::queue::QueuedRender<String>>,
}

/// Route: authority
///
/// Returns the device authority record.
pub async fn authority(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let worker = request.state();
  let user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-user")
  })?;
  let query = request.query::<LookupQuery>()?;
  let (_, model) = worker.user_access(&user.oid, &query.id).await?.ok_or_else(|| {
    log::warn!("'{}' has no access to device '{}'", user.oid, query.id);
    tide::Error::from_str(400, "not-found")
  })?;
  tide::Body::from_json(&model).map(|body| tide::Response::builder(200).body(body).build())
}

/// Route: info
///
/// Returns basic info about the device.
pub async fn info(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let mut now = std::time::Instant::now();
  let worker = request.state();

  let user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-user")
  })?;

  let query = request.query::<LookupQuery>()?;

  if worker.user_access(&user.oid, &query.id).await?.is_none() {
    log::warn!("'{}' has no access to device '{}'", user.oid, query.id);
    return Err(tide::Error::from_str(400, "not-found"));
  }

  log::trace!("user loaded in {}ms", now.elapsed().as_millis());
  now = std::time::Instant::now();

  let current_queue_len = match worker
    .command(&kramer::Command::Lists::<&String, &String>(kramer::ListCommand::Len(
      &crate::redis::device_message_queue_id(&query.id),
    )))
    .await
  {
    Ok(kramer::Response::Item(kramer::ResponseValue::Integer(i))) => i,
    Ok(response) => {
      log::warn!("unrecognized device message queue len response  - {response:?}");
      0
    }
    Err(error) => {
      log::warn!("queue len error - {error:?}");
      0
    }
  };

  log::trace!("device queue len loaded in {}ms", now.elapsed().as_millis());
  now = std::time::Instant::now();

  let device_diagnostic = worker
    .device_diagnostic_collection()?
    .find_one(bson::doc! { "id": &query.id }, None)
    .await
    .map_err(|error| {
      log::warn!("unable to query device diags - {error}");
      tide::Error::from_str(500, "server-error")
    })?
    .ok_or_else(|| {
      log::warn!("unable to find device diag matching");
      tide::Error::from_str(404, "not-found")
    })?;

  log::trace!("device diagnostic loaded in {}ms", now.elapsed().as_millis());

  let info = DeviceInfoPayload {
    id: device_diagnostic.id,
    last_seen: device_diagnostic.last_seen,
    first_seen: device_diagnostic.first_seen,
    sent_message_count: device_diagnostic.sent_message_count,
    current_queue_count: current_queue_len,
    nickname: device_diagnostic.nickname.as_ref().cloned(),

    // This is pending work to migrate the message history from on the diagnostic record itself to
    // somewhere else. This is important to minimize the exposure of breaking changes in the schema
    // of device diagnostics.
    sent_messages: vec![],
  };

  log::trace!("user '{}' fetched device '{}'", user.oid, info.id);
  tide::Body::from_json(&info).map(|body| tide::Response::builder(200).body(body).build())
}

/// Route: unregister
///
/// Removes a device from the user's document in mongo.
pub async fn unregister(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let worker = request.state();
  let users = worker.users_collection()?;

  let mut user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("device unregister -> no user found");
    tide::Error::from_str(404, "missing-user")
  })?;

  let payload = request.body_json::<RegistrationPayload>().await.map_err(|error| {
    log::warn!("invalid request payload - {error}");
    tide::Error::from_str(422, "bad-payload")
  })?;

  match user.devices.take() {
    Some(mut device_map) => {
      log::trace!("device unregister -> found device map - {device_map:?}");

      if device_map.remove(&payload.device_id).is_none() {
        return Ok(tide::Response::builder(422).build());
      }

      // Update our user handle
      let oid = user.oid.clone();
      let query = bson::doc! { "oid": &oid };
      let updated = crate::types::User {
        devices: Some(device_map),
        ..user
      };
      let options = mongodb::options::FindOneAndUpdateOptions::builder()
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .build();

      // Persist update into mongo
      users
        .find_one_and_update(
          query,
          bson::doc! { "$set": bson::to_bson(&updated).map_err(|error| {
            log::warn!("unable to serialize user update - {error}");
            tide::Error::from_str(500, "user-failure")
          })? },
          options,
        )
        .await
        .map_err(|error| {
          log::warn!("unable to create new user - {:?}", error);
          tide::Error::from_str(500, "user-failure")
        })?;

      log::info!("user '{}' unregistered device '{}'", oid, payload.device_id);
      Ok(tide::Response::builder(200).build())
    }
    None => {
      log::warn!("user has no devices, not found");
      Ok(tide::Response::builder(422).build())
    }
  }
}

/// Route: register
///
/// This api route will attempt to parse the request payload and register the device id
/// with the user identified in the request cookie.
pub async fn register(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let payload = request.body_json::<RegistrationPayload>().await.map_err(|error| {
    log::warn!("device-register -> invalid request payload - {error}");
    tide::Error::from_str(422, "bad-payload")
  })?;

  let worker = request.state();

  let user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("device-register -> no user found");
    tide::Error::from_str(404, "missing-user")
  })?;

  let job_id = worker
    .queue_job(crate::registrar::RegistrarJob::device_ownership(
      &user.oid,
      &payload.device_id,
    ))
    .await?;

  log::info!(
    "device-register -> user '{}' registered '{}'",
    user.oid,
    payload.device_id
  );

  tide::Body::from_json(&RegistrationResponse { id: job_id })
    .map(|body| tide::Response::builder(200).body(body).build())
}
