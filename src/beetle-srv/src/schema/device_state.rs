use crate::vendor::google;
use serde::{Deserialize, Serialize};

/// Entries in our device rendering state that are messages.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceRenderingStateMessageEntry {
  /// The string to be rendered.
  pub content: String,
  /// Who/what sent this message.
  pub origin: DeviceStateMessageOrigin,
  /// The timestamp the message was added to our list.
  pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// This schema is the long-lived representation of what is being rendered to a device.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceRenderingState {
  /// The layout of our state for rendering a calendar.
  ScheduleLayout {
    /// The latest list of events that will be rendered.
    events: Vec<google::ParsedEvent>,

    /// The latest list of messages that will be rendered.
    messages: Vec<DeviceRenderingStateMessageEntry>,
  },

  /// Just a list of messages.
  MessageList {
    /// The list of messages.
    messages: Vec<DeviceRenderingStateMessageEntry>,
  },
}

/// This schema is the long-lived representation of what is being rendered to a device.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DeviceState {
  /// The id of a device.
  pub(crate) device_id: String,

  /// The timestamp of the last time we were updated.
  pub(crate) updated_at: Option<chrono::DateTime<chrono::Utc>>,

  /// The render state.
  pub(crate) rendering: Option<DeviceRenderingState>,
}

/// The various kinds of origins messages can come from.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceStateMessageOrigin {
  /// An anonymous message.
  Unknown,

  /// This message came from a user. The string is their nickname.
  User {
    /// The nickname of the user.
    nickname: String,
  },
}
