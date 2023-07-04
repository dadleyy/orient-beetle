//! This is better named as `RenderedDeviceState`, since what we're really concerned with is
//! transitioning the things we want to render to the device from one state to another.

use crate::schema;
use anyhow::Context;
use serde::{Deserialize, Serialize};

/// This type is used internally to these jobs as the schema used for the `$set` payload in our
/// device state update requests.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct PartialStateUpdate {
  /// The timestamp of the last time we were updated.
  updated_at: Option<chrono::DateTime<chrono::Utc>>,

  /// The render state.
  rendering: Option<schema::DeviceRenderingState>,
}

/// The kinds of mutations supported for the rendered device state.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceStateTransition {
  /// Attemps to set the state to be viewing a list of events.
  SetSchedule(Vec<crate::vendor::google::ParsedEvent>),

  /// Attemps to add a message to the device state.
  PushMessage(String, schema::DeviceStateMessageOrigin),
}

/// The device state transition job kind.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceStateTransitionRequest {
  /// The id of a device.
  pub(crate) device_id: String,

  /// The transition.
  pub(crate) transition: DeviceStateTransition,
}

/// Will attempt to build a render layout based on the current state and send it along.
pub(super) async fn render_current(
  mut handle: super::worker::WorkerHandle<'_>,
  device_id: &String,
) -> anyhow::Result<()> {
  log::info!("will render current device state - '{device_id}'");
  let states = handle.device_state_collection()?;

  let current_state = states
    .find_one(bson::doc! { "device_id": &device_id }, None)
    .await
    .with_context(|| format!("unable to load current device state for '{device_id}'"))?;

  log::info!("current render state for '{device_id}': {current_state:?}");

  Ok(())
}

/// Will attempt to run the transition request.
pub(super) async fn attempt_transition(
  mut handle: super::worker::WorkerHandle<'_>,
  transition_request: &DeviceStateTransitionRequest,
) -> anyhow::Result<()> {
  log::info!("attempting to transition {transition_request:?}");
  let states = handle.device_state_collection()?;
  let device_id = transition_request.device_id.clone();

  let current_state = states
    .find_one_and_update(
      bson::doc! { "device_id": &device_id },
      bson::doc! { "$setOnInsert": { "device_id": &device_id } },
      mongodb::options::FindOneAndUpdateOptions::builder()
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .build(),
    )
    .await?
    .with_context(|| format!("unable to find device '{}'", &device_id))?;

  log::debug!("loaded current state for transition - {current_state:?}");

  let next_state = match (current_state.rendering, &transition_request.transition) {
    (None, DeviceStateTransition::PushMessage(content, origin)) => Some(schema::DeviceRenderingState::MessageList(
      vec![(content.clone(), origin.clone())],
    )),
    _ => None,
  };

  let update = bson::to_document(&PartialStateUpdate {
    updated_at: Some(chrono::Utc::now()),
    rendering: next_state,
  })
  .with_context(|| "unable to serialize partial state update")?;

  let updated_state = states
    .find_one_and_update(
      bson::doc! { "device_id": &device_id },
      bson::doc! { "$set": update },
      mongodb::options::FindOneAndUpdateOptions::builder()
        .return_document(mongodb::options::ReturnDocument::After)
        .build(),
    )
    .await?;

  log::debug!("final state - {updated_state:?}");

  let percolated_render_id = handle
    .enqueue_kind(super::jobs::RegistrarJobKind::Renders(
      super::jobs::RegistrarRenderKinds::CurrentDeviceState(device_id),
    ))
    .await?;

  log::info!("percolated render id - '{percolated_render_id}'");

  Ok(())
}
