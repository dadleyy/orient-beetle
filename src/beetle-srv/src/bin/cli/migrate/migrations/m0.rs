#![allow(clippy::missing_docs_in_private_items)]

//! Migration note: This migration is responsible for the breaking changes made to the
//! `device_authorities` and `device_states` collections, which were converted to use nested
//! structs as their enum datum (instead of tuple fields).

use super::super::ops::from_to;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "migration:name")]
pub struct Migration {}

impl Migration {
  pub async fn up(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    log::debug!("moving from old to new");
    from_to(config, &config.mongo.collections.device_schedules, up_schedule).await?;
    from_to(config, &config.mongo.collections.device_states, up_state).await?;
    from_to(config, &config.mongo.collections.device_authorities, up_auth).await
  }

  pub async fn down(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    from_to(config, &config.mongo.collections.device_schedules, down_schedule).await?;
    from_to(config, &config.mongo.collections.device_states, down_state).await?;
    from_to(config, &config.mongo.collections.device_authorities, down_auth).await
  }
}

fn up_auth(record: OriginDeviceAuthorityRecord) -> (TargetDeviceAuthorityRecord, bson::Document) {
  let updated = TargetDeviceAuthorityRecord {
    device_id: record.device_id.clone(),
    authority_model: record.authority_model.map(|model| match model {
      OriginDeviceAuthorityModel::Exclusive(owner) => TargetDeviceAuthorityModel::Exclusive { owner },
      OriginDeviceAuthorityModel::Shared(owner, _) => TargetDeviceAuthorityModel::Shared { owner, guests: vec![] },
      OriginDeviceAuthorityModel::Public(owner, _) => TargetDeviceAuthorityModel::Public { owner, guests: vec![] },
    }),
  };

  (updated, bson::doc! { "device_id": record.device_id })
}

fn down_auth(record: TargetDeviceAuthorityRecord) -> (OriginDeviceAuthorityRecord, bson::Document) {
  let updated = OriginDeviceAuthorityRecord {
    device_id: record.device_id.clone(),
    authority_model: record.authority_model.map(|model| match model {
      TargetDeviceAuthorityModel::Exclusive { owner } => OriginDeviceAuthorityModel::Exclusive(owner),
      TargetDeviceAuthorityModel::Shared { owner, .. } => OriginDeviceAuthorityModel::Exclusive(owner),
      TargetDeviceAuthorityModel::Public { owner, .. } => OriginDeviceAuthorityModel::Exclusive(owner),
    }),
  };

  (updated, bson::doc! { "device_id": record.device_id })
}

fn up_schedule(record: OriginDeviceSchedule) -> (TargetDeviceSchedule, bson::Document) {
  let updated = TargetDeviceSchedule {
    device_id: record.device_id.clone(),
    last_executed: record.last_executed,
    kind: record.kind.map(|k| match k {
      OriginDeviceScheduleKind::UserEventsBasic(id) => TargetDeviceScheduleKind::UserEventsBasic { user_oid: id },
    }),
  };
  log::debug!("migrating UP TO schedule '{updated:?}'");

  (updated, bson::doc! { "device_id": record.device_id })
}

fn down_schedule(record: TargetDeviceSchedule) -> (OriginDeviceSchedule, bson::Document) {
  let updated = OriginDeviceSchedule {
    device_id: record.device_id.clone(),
    last_executed: record.last_executed,
    kind: record.kind.map(|k| match k {
      TargetDeviceScheduleKind::UserEventsBasic { user_oid } => OriginDeviceScheduleKind::UserEventsBasic(user_oid),
    }),
  };
  log::debug!("migrating DOWN TO schedule '{updated:?}'");

  (updated, bson::doc! { "device_id": record.device_id })
}

fn up_state(record: OriginDeviceState) -> (TargetDeviceState, bson::Document) {
  let updated = TargetDeviceState {
    updated_at: record.updated_at,
    device_id: record.device_id.clone(),
    rendering: record.rendering.map(|old| match old {
      OriginDeviceRenderingState::MessageList(messages) => {
        let messages = messages
          .into_iter()
          .map(|m| TargetDeviceRenderingStateMessageEntry {
            content: m.content,
            origin: match m.origin {
              OriginDeviceStateMessageOrigin::Unknown => TargetDeviceStateMessageOrigin::Unknown,
              OriginDeviceStateMessageOrigin::User(id) => TargetDeviceStateMessageOrigin::User { nickname: id },
            },
            timestamp: m.timestamp,
          })
          .collect();
        TargetDeviceRenderingState::MessageList { messages }
      }
      OriginDeviceRenderingState::ScheduleLayout(events, messages) => {
        let messages = messages
          .into_iter()
          .map(|m| TargetDeviceRenderingStateMessageEntry {
            content: m.content,
            origin: match m.origin {
              OriginDeviceStateMessageOrigin::Unknown => TargetDeviceStateMessageOrigin::Unknown,
              OriginDeviceStateMessageOrigin::User(id) => TargetDeviceStateMessageOrigin::User { nickname: id },
            },
            timestamp: m.timestamp,
          })
          .collect();

        TargetDeviceRenderingState::ScheduleLayout { events, messages }
      }
    }),
  };

  (updated, bson::doc! { "device_id": record.device_id })
}

fn down_state(record: TargetDeviceState) -> (OriginDeviceState, bson::Document) {
  let updated = OriginDeviceState {
    updated_at: record.updated_at,
    device_id: record.device_id.clone(),
    rendering: record.rendering.map(|old| match old {
      TargetDeviceRenderingState::MessageList { messages } => {
        let messages = messages
          .into_iter()
          .map(|m| OriginDeviceRenderingStateMessageEntry {
            content: m.content,
            origin: match m.origin {
              TargetDeviceStateMessageOrigin::Unknown => OriginDeviceStateMessageOrigin::Unknown,
              TargetDeviceStateMessageOrigin::User { nickname } => OriginDeviceStateMessageOrigin::User(nickname),
            },
            timestamp: m.timestamp,
          })
          .collect();
        OriginDeviceRenderingState::MessageList(messages)
      }
      TargetDeviceRenderingState::ScheduleLayout { events, messages } => {
        let messages = messages
          .into_iter()
          .map(|m| OriginDeviceRenderingStateMessageEntry {
            content: m.content,
            origin: match m.origin {
              TargetDeviceStateMessageOrigin::Unknown => OriginDeviceStateMessageOrigin::Unknown,
              TargetDeviceStateMessageOrigin::User { nickname } => OriginDeviceStateMessageOrigin::User(nickname),
            },
            timestamp: m.timestamp,
          })
          .collect();

        OriginDeviceRenderingState::ScheduleLayout(events, messages)
      }
    }),
  };

  (updated, bson::doc! { "device_id": record.device_id })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum TargetDeviceStateMessageOrigin {
  Unknown,
  User { nickname: String },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct TargetDeviceRenderingStateMessageEntry {
  content: String,
  origin: TargetDeviceStateMessageOrigin,
  timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum TargetDeviceRenderingState {
  ScheduleLayout {
    events: Vec<beetle::vendor::google::ParsedEvent>,
    messages: Vec<TargetDeviceRenderingStateMessageEntry>,
  },
  MessageList {
    messages: Vec<TargetDeviceRenderingStateMessageEntry>,
  },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum OriginDeviceStateMessageOrigin {
  Unknown,
  User(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct OriginDeviceRenderingStateMessageEntry {
  content: String,
  origin: OriginDeviceStateMessageOrigin,
  timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum OriginDeviceRenderingState {
  ScheduleLayout(
    Vec<beetle::vendor::google::ParsedEvent>,
    Vec<OriginDeviceRenderingStateMessageEntry>,
  ),
  MessageList(Vec<OriginDeviceRenderingStateMessageEntry>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct TargetDeviceState {
  device_id: String,
  updated_at: Option<chrono::DateTime<chrono::Utc>>,
  rendering: Option<TargetDeviceRenderingState>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct OriginDeviceState {
  device_id: String,
  updated_at: Option<chrono::DateTime<chrono::Utc>>,
  rendering: Option<OriginDeviceRenderingState>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum OriginDeviceAuthorityModel {
  Exclusive(String),
  Shared(String, Vec<String>),
  Public(String, Vec<String>),
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
struct OriginDeviceAuthorityRecord {
  device_id: String,
  authority_model: Option<OriginDeviceAuthorityModel>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum TargetDeviceAuthorityModel {
  Exclusive { owner: String },
  Shared { owner: String, guests: Vec<String> },
  Public { owner: String, guests: Vec<String> },
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
struct TargetDeviceAuthorityRecord {
  device_id: String,
  authority_model: Option<TargetDeviceAuthorityModel>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum OriginDeviceScheduleKind {
  UserEventsBasic(String),
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
struct OriginDeviceSchedule {
  device_id: String,
  last_executed: Option<u64>,
  kind: Option<OriginDeviceScheduleKind>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
enum TargetDeviceScheduleKind {
  UserEventsBasic { user_oid: String },
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
struct TargetDeviceSchedule {
  device_id: String,
  last_executed: Option<u64>,
  kind: Option<TargetDeviceScheduleKind>,
}
