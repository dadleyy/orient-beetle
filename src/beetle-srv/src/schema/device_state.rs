use serde::{Deserialize, Serialize};

/// This schema is the long-lived representation of what is being rendered to a device.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum DeviceRenderingState {
  /// The layout of our state for rendering a calendar.
  ScheduleLayout,

  /// Just a list of messages.
  MessageList(Vec<(String, DeviceStateMessageOrigin)>),
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
  User(String),
}
