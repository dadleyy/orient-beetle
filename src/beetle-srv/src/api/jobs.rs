//! Defines routes for job lookup as well as queing.

use crate::{registrar, schema};
use serde::{Deserialize, Serialize};

/// The max allowed file upload size, in bytes
const MAX_FILE_SIZE: u32 = 1_000_000 * 5;

/// The payload for looking up a device by id.
#[derive(Debug, Deserialize)]
struct LookupQuery {
  /// The id of a device in question.
  id: String,
}

/// The api used to add various layouts to a device queue.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct QueuePayload {
  /// The id of the device.
  device_id: String,
  /// The contents of the message.
  kind: QueuePayloadKind,
}

/// The schema of responses sent from the registration api.
#[derive(Debug, Serialize)]
struct QueueResponse {
  /// The id of the device registered.
  id: String,
}

/// The api wrapper around convenience types for the underlying layout kinds.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum QueuePayloadKind {
  /// Controls the lights.
  Lights(bool),

  /// Enables or disables the automated device schedule for the current user. Eventually this
  /// should become much more parameterized, instead of a simple on/off.
  Schedule(bool),

  /// Renders text.
  Message(String),

  /// Will queue a QR code render for the device.
  Link(String),

  /// Attempts to rename the device.
  Rename(String),

  /// Attempts to render the currently persisted state for a device.
  Refresh,

  /// Attempts to clear anything that is currently rendered.
  ClearRender,

  /// Attempts to queue a message that will force redisplay of registration.
  Registration,

  /// Will update the ownership record model to be public. This should be combined with the
  /// `MakePrivate` variant, but expressing with a boolean on some `Toggle` variant always leads to
  /// the confusion of what `true` means.
  MakePublic,

  /// Will update the ownership record model to be private.
  MakePrivate,
}

/// Route: message
///
/// Sends a message to the device.
pub async fn queue(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  // TODO: we're scoping this to make the immutable borrow of the worker only last while we load
  // the user. The borrow cannot live longer than this because we need a mutable borrow to read the
  // body.
  let user = {
    let worker = request.state();
    worker
      .request_authority(&request)
      .await?
      .ok_or_else(|| {
        log::warn!("no user found");
        tide::Error::from_str(404, "missing-user")
      })
      .map_err(|error| {
        log::warn!("unable to determine request authority - {error}");
        error
      })?
  };

  let content_type = request
    .content_type()
    .ok_or_else(|| tide::Error::from_str(422, "missing content-type"))?;

  match content_type.essence() {
    image_kind @ "image/jpeg" | image_kind @ "image/png" => {
      let device_id = request
        .param("device_id")?
        // .ok_or_else(|| tide::Error::from_str(422, "bad-id"))
        .to_string();

      // TODO: borrow scoping...
      {
        let worker = request.state();
        if worker.user_access(&user.oid, &device_id).await?.is_none() {
          return Err(tide::Error::from_str(404, "not-found"));
        }
      }

      let size = request.len().ok_or_else(|| {
        log::warn!("unable to determine image size from upload");
        tide::Error::from_str(422, "missing image upload size")
      })?;

      if (size as u32) > MAX_FILE_SIZE {
        log::warn!("invalid image upload size: '{size}'");
        return Err(tide::Error::from_str(422, "image too large"));
      }

      log::debug!("has image upload for device queue '{device_id}' of {size} bytes");
      let mut bytes = request.take_body();
      let mut storage_dest = std::path::PathBuf::new();
      storage_dest.push(&request.state().web_configuration.temp_file_storage);
      async_std::fs::create_dir_all(&storage_dest).await.map_err(|error| {
        log::error!("unable to ensure temporary file storage dir exists - {error}");
        tide::Error::from_str(500, "bad")
      })?;
      let file_name = uuid::Uuid::new_v4().to_string();

      storage_dest.push(&file_name);
      storage_dest.set_extension(if image_kind == "image/jpeg" { "jpg" } else { "png" });

      log::info!("writing temporary file to '{storage_dest:?}");
      let mut file = async_std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&storage_dest)
        .await
        .map_err(|error| {
          log::error!("unable to create temporary file for upload - {error}");
          tide::Error::from_str(500, "bad")
        })?;

      async_std::io::copy(&mut bytes, &mut file).await.map_err(|error| {
        log::error!("unable to copy file upload - {error}");
        tide::Error::from_str(500, "bad")
      })?;

      let job = registrar::RegistrarJobKind::Renders(registrar::jobs::RegistrarRenderKinds::SendImage {
        location: storage_dest.to_string_lossy().to_string(),
        device_id,
      });
      let worker = request.state();
      let id = worker.queue_job_kind(job).await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }
    other => {
      log::warn!("strange content type - '{other}'");
    }
  }

  let queue_payload = request.body_json::<QueuePayload>().await.map_err(|error| {
    log::warn!("bad device message payload - {error}");
    tide::Error::from_str(422, "bad-request")
  })?;

  let worker = request.state();
  if worker.user_access(&user.oid, &queue_payload.device_id).await?.is_none() {
    log::warn!("'{}' has no access to device '{}'", user.oid, queue_payload.device_id);
    return Err(tide::Error::from_str(400, "not-found"));
  }

  log::info!("queue payload request received - {queue_payload:?}");

  let device_id = queue_payload.device_id.clone();

  log::info!(
    "user '{}' creating message for device '{device_id}' - {:?}",
    user.oid,
    queue_payload.kind
  );

  let layout = match queue_payload.kind {
    kind @ QueuePayloadKind::MakePublic | kind @ QueuePayloadKind::MakePrivate => {
      let privacy = match kind {
        QueuePayloadKind::MakePublic => registrar::ownership::PublicAvailabilityChange::ToPublic,
        QueuePayloadKind::MakePrivate => registrar::ownership::PublicAvailabilityChange::ToPrivate,
        _ => return Ok(tide::Error::from_str(422, "bad transition").into()),
      };

      let job = registrar::RegistrarJobKind::OwnershipChange(
        registrar::ownership::DeviceOwnershipChangeRequest::SetPublicAvailability(device_id, privacy),
      );
      let id = worker.queue_job_kind(job).await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }

    QueuePayloadKind::ClearRender => {
      log::info!("clearing current device '{device_id}' render state!");
      let id = worker
        .queue_job_kind(registrar::RegistrarJobKind::MutateDeviceState(
          registrar::device_state::DeviceStateTransitionRequest {
            device_id,
            transition: registrar::device_state::DeviceStateTransition::Clear,
          },
        ))
        .await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }

    QueuePayloadKind::Refresh => {
      log::debug!("refreshing device state for '{device_id}'");
      let job =
        registrar::RegistrarJobKind::Renders(registrar::jobs::RegistrarRenderKinds::CurrentDeviceState(device_id));
      let id = worker.queue_job_kind(job).await?;
      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }

    // Attempt to queue the large, scannable QR code for registering this define.
    QueuePayloadKind::Registration => {
      let job =
        registrar::RegistrarJobKind::Renders(registrar::jobs::RegistrarRenderKinds::RegistrationScannable(device_id));
      let id = worker.queue_job_kind(job).await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }

    QueuePayloadKind::Schedule(desired_state) => {
      log::info!("toggling device schedule '{desired_state:?}' for user '{}'", user.oid);
      let user_id = user.oid.clone();
      let id = worker
        .queue_job_kind(registrar::RegistrarJobKind::ToggleDefaultSchedule {
          device_id,
          user_id,
          should_enable: desired_state,
        })
        .await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }
    QueuePayloadKind::Rename(new_name) => {
      let id = worker
        .queue_job_kind(registrar::RegistrarJobKind::Rename(registrar::DeviceRenameRequest {
          device_id,
          new_name,
        }))
        .await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }

    // Attempt to transition the device rendering state, which will cause a re-render.
    QueuePayloadKind::Message(message) => {
      let origin = user
        .nickname
        .or(user.name)
        .map(|name| schema::DeviceStateMessageOrigin::User { nickname: name })
        .unwrap_or_else(|| schema::DeviceStateMessageOrigin::Unknown);

      let id = worker
        .queue_job_kind(registrar::RegistrarJobKind::MutateDeviceState(
          registrar::device_state::DeviceStateTransitionRequest {
            device_id,
            transition: registrar::device_state::DeviceStateTransition::PushMessage(message, origin),
          },
        ))
        .await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }

    // TODO: these variants need to go through the device state transition flow; they are currently
    // being written directly to the device render queue here.
    QueuePayloadKind::Lights(true) => crate::rendering::RenderVariant::on(),
    QueuePayloadKind::Lights(false) => crate::rendering::RenderVariant::off(),
    QueuePayloadKind::Link(scannable_link) => crate::rendering::RenderVariant::scannable(scannable_link),
  };

  log::debug!("immediately requesting render for '{device_id}' from api");

  let request_id = worker
    .queue_render(&device_id, &user.oid, layout)
    .await
    .map_err(|error| {
      log::warn!(
        "unable to queue render for device '{}' -> '{error}'",
        queue_payload.device_id
      );
      error
    })?;

  tide::Body::from_json(&QueueResponse { id: request_id }).map(|body| tide::Response::builder(200).body(body).build())
}

/// Attempts to find a job result based on the id of the job provided in the query params.
pub async fn find(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let query = request.query::<LookupQuery>().map_err(|error| {
    log::warn!("invalid job lookup - {error}");
    tide::Error::from_str(422, "missing-id")
  })?;
  let worker = request.state();
  worker
    .request_authority(&request)
    .await?
    .ok_or_else(|| {
      log::warn!("no user found");
      tide::Error::from_str(404, "missing-user")
    })
    .map_err(|error| {
      log::warn!("unable to determine request authority - {error}");
      error
    })?;

  log::debug!("attempting to find result for job '{}'", query.id);

  let res = worker
    .command::<&str, &str>(&kramer::Command::Hashes(kramer::HashCommand::Get(
      crate::constants::REGISTRAR_JOB_RESULTS,
      Some(kramer::Arity::One(&query.id)),
    )))
    .await
    .map_err(|error| {
      log::warn!("unable to lookup job - {error}");
      tide::Error::from_str(500, "internal error")
    })?;

  match res {
    kramer::Response::Item(kramer::ResponseValue::Empty) => Ok(tide::Response::builder(404).build()),
    kramer::Response::Item(kramer::ResponseValue::String(contents)) => {
      log::debug!("found job contents - '{contents:?}'");
      let parsed = serde_json::from_str::<schema::jobs::JobResult>(&contents).map_err(|error| {
        log::warn!("unable to lookup job - {error}");
        tide::Error::from_str(500, "internal error")
      })?;

      tide::Body::from_json(&parsed).map(|body| tide::Response::builder(200).body(body).build())
    }
    other => {
      log::warn!("unable to lookup job - {other:?}");
      Err(tide::Error::from_str(500, "internal error"))
    }
  }
}
