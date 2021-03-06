use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct LookupQuery {
  id: String,
}

#[derive(Debug, Deserialize)]
struct MessagePayload {
  device_id: String,
  message: String,
}

#[derive(Debug, Deserialize)]
struct RegistrationPayload {
  device_id: String,
}

#[derive(Debug, Serialize)]
struct RegistrationResponse {
  id: String,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct DeviceInfoPayload {
  id: String,
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  first_seen: Option<chrono::DateTime<chrono::Utc>>,
  #[serde(with = "chrono::serde::ts_milliseconds_option")]
  last_seen: Option<chrono::DateTime<chrono::Utc>>,
  sent_message_count: Option<u32>,
  current_queue_count: i64,
}

async fn parse_message(request: &mut tide::Request<super::worker::Worker>) -> tide::Result<MessagePayload> {
  request.body_json::<MessagePayload>().await
}

/// Route: message
///
/// Sends a message to the device.
pub async fn message(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let devices = request.state().device_diagnostic_collection()?;
  let user = request.state().request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-player")
  })?;
  let body = parse_message(&mut request).await.map_err(|error| {
    log::warn!("bad device message payload - {error}");
    tide::Error::from_str(422, "bad-request")
  })?;

  if user
    .devices
    .as_ref()
    .map(|list| list.contains_key(&body.device_id))
    .unwrap_or(false)
    == false
  {
    log::warn!("'{}' has no access to device '{}'", user.oid, body.device_id);
    return Err(tide::Error::from_str(400, "not-found"));
  }

  log::debug!("user {:?} creating message for device - {body:?}", user);

  request
    .state()
    .command(&kramer::Command::List(kramer::ListCommand::Push(
      (kramer::Side::Left, kramer::Insertion::Always),
      crate::redis::device_message_queue_id(&body.device_id),
      kramer::Arity::One(body.message),
    )))
    .await?;

  if let Err(error) = devices
    .update_one(
      bson::doc! { "id": &body.device_id },
      bson::doc! { "$inc": { "sent_message_count": 1 } },
      None,
    )
    .await
  {
    log::warn!("unable to update device diagnostic total message count - {error}");
  }

  tide::Body::from_json(&RegistrationResponse { id: body.device_id })
    .map(|body| tide::Response::builder(200).body(body).build())
}

/// Route: info
///
/// Returns basic info about the device.
pub async fn info(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let mut now = std::time::Instant::now();
  let worker = request.state();

  let user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-player")
  })?;

  let query = request.query::<LookupQuery>()?;

  if user
    .devices
    .as_ref()
    .map(|list| list.contains_key(&query.id))
    .unwrap_or(false)
    == false
  {
    log::warn!("'{}' has no access to device '{}'", user.oid, query.id);
    return Err(tide::Error::from_str(400, "not-found"));
  }

  log::debug!("user loaded in {}ms", now.elapsed().as_millis());
  now = std::time::Instant::now();

  let current_queue_len = match worker
    .command(&kramer::Command::List::<&String, &String>(kramer::ListCommand::Len(
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

  log::debug!("device queue len loaded in {}ms", now.elapsed().as_millis());
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

  log::debug!("device diagnostic loaded in {}ms", now.elapsed().as_millis());

  let info = DeviceInfoPayload {
    id: device_diagnostic.id,
    last_seen: device_diagnostic.last_seen,
    first_seen: device_diagnostic.first_seen,
    sent_message_count: device_diagnostic.sent_message_count,
    current_queue_count: current_queue_len,
  };

  tide::Body::from_json(&info).map(|body| tide::Response::builder(200).body(body).build())
}

/// Route: unregister
///
/// Removes a device from the user's document in mongo.
pub async fn unregister(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let worker = request.state();
  let users = worker.users_collection()?;

  let mut user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-player")
  })?;

  let payload = request.body_json::<RegistrationPayload>().await.map_err(|error| {
    log::warn!("invalid request payload - {error}");
    tide::Error::from_str(422, "bad-payload")
  })?;

  match user.devices.take() {
    Some(mut device_map) => {
      log::debug!("found device map - {device_map:?}");

      if device_map.remove(&payload.device_id).is_some() == false {
        return Ok(tide::Response::builder(422).build());
      }

      // Update our user handle
      let query = bson::doc! { "oid": user.oid.clone() };
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
            tide::Error::from_str(500, "player-failure")
          })? },
          options,
        )
        .await
        .map_err(|error| {
          log::warn!("unable to create new player - {:?}", error);
          tide::Error::from_str(500, "player-failure")
        })?;

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
  let worker = request.state();
  let devices = worker.device_diagnostic_collection()?;
  let users = worker.users_collection()?;

  let mut user = worker.request_authority(&request).await?.ok_or_else(|| {
    log::warn!("no user found");
    tide::Error::from_str(404, "missing-player")
  })?;

  let payload = request.body_json::<RegistrationPayload>().await.map_err(|error| {
    log::warn!("invalid request payload - {error}");
    tide::Error::from_str(422, "bad-payload")
  })?;

  let found_device = devices.find_one(bson::doc! { "id": &payload.device_id }, None).await;

  match found_device {
    Ok(Some(diagnostic)) => log::info!("found device for registration - {diagnostic:?}"),
    Ok(None) => {
      log::warn!("unable to find '{}'", payload.device_id);
      return Err(tide::Error::from_str(404, "not-found"));
    }
    Err(error) => {
      log::warn!(
        "unable to query for '{}' during registration - {error}",
        payload.device_id
      );
      return Err(tide::Error::from_str(404, "not-found"));
    }
  }

  let query = bson::doc! { "oid": user.oid.clone() };

  // Update or create our new devices hash for this user.
  let devices = user
    .devices
    .take()
    .map(|mut existing_devices| {
      existing_devices.insert(payload.device_id.clone(), 1);
      existing_devices
    })
    .or_else(|| {
      let mut start = std::collections::HashMap::with_capacity(1);
      start.insert(payload.device_id.clone(), 0);
      Some(start)
    });

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
        tide::Error::from_str(500, "player-failure")
      })? },
      options,
    )
    .await
    .map_err(|error| {
      log::warn!("unable to create new player - {:?}", error);
      tide::Error::from_str(500, "player-failure")
    })?;

  tide::Body::from_json(&RegistrationResponse { id: payload.device_id })
    .map(|body| tide::Response::builder(200).body(body).build())
}
