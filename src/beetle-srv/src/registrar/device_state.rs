//! This is better named as `RenderedDeviceState`, since what we're really concerned with is
//! transitioning the things we want to render to the device from one state to another.

use crate::{rendering, schema, vendor::google};
use anyhow::Context;
use serde::{Deserialize, Serialize};

/// The most amount of events to display at once when rendering things from a google calendar.
const MAX_DISPLAYED_EVENTS: usize = 4;

/// The most amount of messages to retain in a list. Older messages are popped off.
const MAX_MESSAGE_LIST_LEN: usize = 4;

/// The size of "secondary" text on the screen.
const SECONDARY_TEXT_SIZE: f32 = 24.0f32;

/// The size of "primary" text on the screen.
const PRIMARY_TEXT_SIZE: f32 = 34.0f32;

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
  /// Attemps to clear the current render state.
  Clear,

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

/// Applies some basic styling for all our components so we don't repeat ourselves too much.
fn apply_padding<S>(component: &mut rendering::components::StylizedMessage<S>) {
  component.padding = Some(rendering::OptionalBoundingBox {
    left: Some(10),
    ..rendering::OptionalBoundingBox::default()
  });
  component.border = Some(rendering::OptionalBoundingBox {
    left: Some(2),
    ..rendering::OptionalBoundingBox::default()
  });
}

/// When rendering messages, we'll either be rendering the origin and timestamp on the same line,
/// or we will split them into two separate lines.
enum MessageEntryLayout {
  /// Render the timestamp and origin separately.
  Separate,
  /// Render the timestamp and origin together.
  Together,
}

/// Pushes some messages onto an accumulator, based on the layout desired.
fn render_message_entry(
  entry: &schema::DeviceRenderingStateMessageEntry,
  acc: &mut Vec<rendering::StylizedMessage<String>>,
  layout: MessageEntryLayout,
) -> () {
  let is_first = acc.is_empty();

  // Render the main content.
  let mut message_component = rendering::components::StylizedMessage::default();
  message_component.message = entry.content.clone();
  message_component.size = Some(PRIMARY_TEXT_SIZE);
  apply_padding(&mut message_component);
  if is_first {
    message_component.margin = Some(rendering::OptionalBoundingBox {
      top: Some(10),
      ..Default::default()
    });
  }
  acc.push(message_component);

  let mut origin_component = rendering::components::StylizedMessage::default();
  origin_component.size = Some(SECONDARY_TEXT_SIZE);
  apply_padding(&mut origin_component);
  origin_component.margin = Some(rendering::OptionalBoundingBox {
    bottom: Some(10),
    ..rendering::OptionalBoundingBox::default()
  });

  // Render the origin information.
  match layout {
    MessageEntryLayout::Separate => {
      origin_component.message = match &entry.origin {
        schema::DeviceStateMessageOrigin::Unknown => "unknown".to_string(),
        schema::DeviceStateMessageOrigin::User(value) => value.clone(),
      };

      if let Some(ts) = entry.timestamp {
        origin_component.margin = None;
        let mut time_component = rendering::StylizedMessage {
          message: ts.format("%B %d, %H:%M").to_string(),
          size: Some(SECONDARY_TEXT_SIZE),
          margin: Some(rendering::OptionalBoundingBox {
            bottom: Some(10),
            ..rendering::OptionalBoundingBox::default()
          }),
          ..rendering::components::StylizedMessage::default()
        };

        apply_padding(&mut time_component);
        acc.push(origin_component);
        acc.push(time_component);
        return ();
      }

      acc.push(origin_component);
    }
    MessageEntryLayout::Together => {
      let from_addr = match &entry.origin {
        schema::DeviceStateMessageOrigin::Unknown => "unknown".to_string(),
        schema::DeviceStateMessageOrigin::User(value) => value.clone(),
      };
      origin_component.message = entry
        .timestamp
        .map(|ts| format!("{from_addr} (@ {})", ts.format("%B %d, %H:%M")))
        .unwrap_or(from_addr);
      acc.push(origin_component);
    }
  }
}

/// This method will actually build the render layout based on the current device rendering state.
/// It is possible that this would be better implemented as an associated method on the
/// `DeviceRenderingState` type itself, but the goal is to avoid _any_ methods directly built in
/// the `schema` module (though it is tempting).
fn render_state(state: &schema::DeviceRenderingState) -> anyhow::Result<rendering::RenderLayout<String>> {
  match state {
    schema::DeviceRenderingState::ScheduleLayout(events, message_list) => {
      let mut left = vec![];

      for event in events.iter().take(MAX_DISPLAYED_EVENTS) {
        log::info!("rendering event '{event:?}'");

        left.push(rendering::components::StylizedMessage {
          message: event.summary.clone(),
          size: Some(PRIMARY_TEXT_SIZE),

          border: Some(rendering::OptionalBoundingBox {
            left: Some(2),
            ..Default::default()
          }),
          margin: Some(rendering::OptionalBoundingBox {
            top: Some(10),
            left: Some(10),
            ..Default::default()
          }),
          padding: Some(rendering::OptionalBoundingBox {
            left: Some(10),
            ..Default::default()
          }),

          ..Default::default()
        });

        match (&event.start, &event.end) {
          (google::ParsedEventTimeMarker::DateTime(s), google::ParsedEventTimeMarker::DateTime(e)) => {
            let formatted_start = s.format("%H:%M").to_string();
            let formatted_end = e.format("%H:%M").to_string();

            left.push(rendering::components::StylizedMessage {
              message: format!("{formatted_start} - {formatted_end}"),
              size: Some(SECONDARY_TEXT_SIZE),

              border: Some(rendering::OptionalBoundingBox {
                left: Some(2),
                ..Default::default()
              }),
              margin: Some(rendering::OptionalBoundingBox {
                left: Some(10),
                ..Default::default()
              }),
              padding: Some(rendering::OptionalBoundingBox {
                left: Some(10),
                ..Default::default()
              }),

              ..Default::default()
            });
          }
          (s, e) => {
            log::warn!("event start/end combination not implemented yet - {s:?} {e:?}");
          }
        }
      }

      let messages = message_list
        .iter()
        .fold(Vec::with_capacity(message_list.len() * 2), |mut acc, entry| {
          render_message_entry(&entry, &mut acc, MessageEntryLayout::Separate);
          acc
        });

      let left = rendering::SplitContents::Messages(left);
      let right = rendering::SplitContents::Messages(messages);
      let split = rendering::SplitLayout { left, right, ratio: 50 };
      Ok(rendering::RenderLayout::Split(split))
    }
    schema::DeviceRenderingState::MessageList(list) => {
      let messages = list.iter().fold(Vec::with_capacity(list.len() * 2), |mut acc, entry| {
        render_message_entry(&entry, &mut acc, MessageEntryLayout::Together);
        acc
      });
      let left = rendering::SplitContents::Messages(messages);
      let right = rendering::SplitContents::Messages(vec![]);
      let split = rendering::SplitLayout { left, right, ratio: 80 };
      Ok(rendering::RenderLayout::Split(split))
    }
  }
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
    .with_context(|| format!("unable to load current device state for '{device_id}'"))?
    .ok_or_else(|| anyhow::Error::msg(format!("no device state found for '{device_id}'")))?;

  log::info!("current render state for '{device_id}': {current_state:?}");

  let layout = current_state
    .rendering
    .as_ref()
    .and_then(|s| {
      render_state(s)
        .map_err(|error| {
          log::error!("was unable to create layout for state - {error}");
          error
        })
        .ok()
    })
    .unwrap_or(rendering::RenderLayout::Clear);

  log::info!("device '{device_id}' attempting to render '{layout:?}'");

  let render_id = handle.render(device_id, layout).await?;

  log::info!("render '{render_id}' scheduled for device '{device_id}'");

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
    .await
    .or_else(|error| {
      if let mongodb::error::ErrorKind::BsonDeserialization(_) = error.kind.as_ref() {
        log::warn!(
          "unable to deserialize current state, will fallback. {error} (kind: {:?})",
          error.kind
        );
        return Ok(Some(schema::DeviceState {
          device_id: device_id.clone(),
          updated_at: None,
          rendering: None,
        }));
      }

      log::error!("bad serialization for device state '{device_id}' - {error:?}");
      Err(error)
    })?
    .ok_or_else(|| anyhow::Error::msg(format!("unable to find device '{}'", &device_id)))?;

  log::debug!("loaded current state for transition - {current_state:?}");

  let next_state = match (current_state.rendering, &transition_request.transition) {
    // push a message onto nothing.
    (None, DeviceStateTransition::PushMessage(content, origin)) => {
      Some(schema::DeviceRenderingState::MessageList(vec![
        schema::DeviceRenderingStateMessageEntry {
          content: content.clone(),
          origin: origin.clone(),
          timestamp: Some(chrono::Utc::now()),
        },
      ]))
    }

    // push a message onto a message list.
    (
      Some(schema::DeviceRenderingState::MessageList(mut current_list)),
      DeviceStateTransition::PushMessage(content, origin),
    ) => {
      while current_list.len() > MAX_MESSAGE_LIST_LEN {
        current_list.pop();
      }
      current_list.push(schema::DeviceRenderingStateMessageEntry {
        content: content.clone(),
        origin: origin.clone(),
        timestamp: Some(chrono::Utc::now()),
      });
      Some(schema::DeviceRenderingState::MessageList(current_list))
    }

    // push a message onto a schedule.
    (
      Some(schema::DeviceRenderingState::ScheduleLayout(events, mut current_list)),
      DeviceStateTransition::PushMessage(content, origin),
    ) => {
      while current_list.len() > MAX_MESSAGE_LIST_LEN {
        current_list.pop();
      }
      current_list.push(schema::DeviceRenderingStateMessageEntry {
        content: content.clone(),
        origin: origin.clone(),
        timestamp: Some(chrono::Utc::now()),
      });

      Some(schema::DeviceRenderingState::ScheduleLayout(events, current_list))
    }

    (_, DeviceStateTransition::Clear) => {
      log::warn!("clearing device '{device_id}' render state!");
      None
    }

    // set schedule onto an existing schedule.
    (Some(schema::DeviceRenderingState::ScheduleLayout(_, messages)), DeviceStateTransition::SetSchedule(events)) => {
      Some(schema::DeviceRenderingState::ScheduleLayout(events.clone(), messages))
    }

    // set schedule onto anything (loss of messages).
    (_, DeviceStateTransition::SetSchedule(events)) => {
      Some(schema::DeviceRenderingState::ScheduleLayout(events.clone(), vec![]))
    }
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
