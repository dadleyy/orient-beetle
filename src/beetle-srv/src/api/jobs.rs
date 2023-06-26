use serde::{Deserialize, Serialize};

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

  /// Attempts to queue a message that will force redisplay of registration.
  Registration,

  /// Will update the ownership record model to be public. This should be combined with the
  /// `MakePrivate` variant, but expressing with a boolean on some `Toggle` variant always leads to
  /// the confusion of what `true` means.
  MakePublic,

  /// Will update the ownership record model to be private.
  MakePrivate,

  /// Clears screen.
  Clear,
}

/// Builds the message layout that is rendered for requests to the job api.
fn message_layout<'a, 'b>(user: &'a crate::types::User, message: &'b str) -> crate::rendering::RenderVariant<String>
where
  'a: 'b,
{
  let left_message = crate::rendering::components::StylizedMessage {
    message: message.to_owned(),
    border: None,
    font: None,
    padding: None,
    margin: None,
    size: Some(36.0f32),
  };

  let timestamp_line = crate::rendering::components::StylizedMessage {
    message: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),

    border: Some(crate::rendering::components::OptionalBoundingBox {
      left: Some(2),
      ..Default::default()
    }),
    font: None,
    padding: Some(crate::rendering::components::OptionalBoundingBox {
      left: Some(10),
      top: Some(10),
      bottom: Some(10),
      ..Default::default()
    }),
    margin: Some(crate::rendering::components::OptionalBoundingBox {
      top: Some(5),
      ..Default::default()
    }),
    size: Some(24.0f32),
  };

  let from_line = crate::rendering::components::StylizedMessage {
    message: user
      .nickname
      .as_ref()
      .or(user.name.as_ref())
      .map(|v| v.as_str())
      .unwrap_or("unknown")
      .to_string(),
    border: Some(crate::rendering::components::OptionalBoundingBox {
      left: Some(2),
      ..Default::default()
    }),
    font: None,
    padding: Some(crate::rendering::components::OptionalBoundingBox {
      left: Some(10),
      top: Some(10),
      bottom: Some(10),
      ..Default::default()
    }),
    margin: Some(crate::rendering::components::OptionalBoundingBox {
      top: Some(200),
      ..Default::default()
    }),
    size: Some(24.0f32),
  };

  let layout = crate::rendering::RenderLayout::Split(crate::rendering::SplitLayout {
    left: crate::rendering::SplitContents::Messages(vec![left_message]),
    right: crate::rendering::SplitContents::Messages(vec![from_line, timestamp_line]),
    ratio: 66,
  });
  let container = crate::rendering::RenderLayoutContainer {
    layout,
    created: Some(chrono::Utc::now()),
  };

  crate::rendering::RenderVariant::Layout(container)
}

/// Route: message
///
/// Sends a message to the device.
pub async fn queue(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let queue_payload = request.body_json::<QueuePayload>().await.map_err(|error| {
    log::warn!("bad device message payload - {error}");
    tide::Error::from_str(422, "bad-request")
  })?;
  log::info!("queue payload request received - {queue_payload:?}");

  let worker = request.state();

  let user = worker
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

  if worker.user_access(&user.oid, &queue_payload.device_id).await?.is_none() {
    log::warn!("'{}' has no access to device '{}'", user.oid, queue_payload.device_id);
    return Err(tide::Error::from_str(400, "not-found"));
  }

  log::info!(
    "user '{}' creating message for device - {:?}",
    user.oid,
    queue_payload.kind
  );

  let device_id = queue_payload.device_id.clone();
  let layout = match queue_payload.kind {
    kind @ QueuePayloadKind::MakePublic | kind @ QueuePayloadKind::MakePrivate => {
      let privacy = match kind {
        QueuePayloadKind::MakePublic => crate::registrar::ownership::PublicAvailabilityChange::ToPublic,
        QueuePayloadKind::MakePrivate => crate::registrar::ownership::PublicAvailabilityChange::ToPrivate,
        _ => return Ok(tide::Error::from_str(422, "bad transition").into()),
      };

      let job = crate::registrar::RegistrarJob::set_public_availability(device_id, privacy);
      let id = worker.queue_job(job).await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }
    QueuePayloadKind::Registration => {
      let job = crate::registrar::RegistrarJob::registration_scannable(device_id);
      let id = worker.queue_job(job).await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }
    QueuePayloadKind::Schedule(desired_state) => {
      log::info!("toggling device schedule '{desired_state:?}' for user '{}'", user.oid);
      let user_id = user.oid.clone();
      let id = worker
        .queue_job_kind(crate::registrar::RegistrarJobKind::ToggleDefaultSchedule {
          device_id,
          user_id,
          should_enable: desired_state,
        })
        .await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }
    QueuePayloadKind::Rename(new_name) => {
      let job = crate::registrar::RegistrarJob::rename_device(device_id, new_name.clone());
      let id = worker.queue_job(job).await?;

      return tide::Body::from_json(&QueueResponse { id }).map(|body| tide::Response::builder(200).body(body).build());
    }
    QueuePayloadKind::Lights(true) => crate::rendering::RenderVariant::on(),
    QueuePayloadKind::Lights(false) => crate::rendering::RenderVariant::off(),
    QueuePayloadKind::Clear => crate::rendering::RenderVariant::message("".to_string()),
    QueuePayloadKind::Link(scannable_link) => crate::rendering::RenderVariant::scannable(scannable_link),
    QueuePayloadKind::Message(message) => message_layout(&user, &message),
  };

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
      let parsed = serde_json::from_str::<crate::registrar::jobs::JobResult>(&contents).map_err(|error| {
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
