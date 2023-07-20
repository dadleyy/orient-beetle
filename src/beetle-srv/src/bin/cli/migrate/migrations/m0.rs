#![allow(clippy::missing_docs_in_private_items)]
use anyhow::Context;
use async_std::stream::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "migration:name")]
pub struct Migration {}

impl Migration {
  pub async fn up(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    log::debug!("moving from old to new");
    self.up_auth(config).await?;
    self.up_states(config).await
  }

  async fn up_states(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
    let db = mongo.database(&config.mongo.database);
    let origin_collection = db.collection::<OriginDeviceState>(&config.mongo.collections.device_states);
    let target_collection = db.collection::<TargetDeviceState>(&config.mongo.collections.device_states);

    let mut cursor = origin_collection
      .find(bson::doc! { "device_id": { "$exists": 1 } }, None)
      .await?;

    let mut updates = vec![];
    while let Some(n) = cursor.next().await {
      log::debug!("attemping to migrate device state '{n:?}'");
      let record = n.with_context(|| "unable to deserialize into old")?;

      let updated = TargetDeviceState {
        updated_at: record.updated_at,
        device_id: record.device_id,
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
      updates.push(updated);
    }

    for update in updates {
      log::debug!("applying update '{update:?}'");
      target_collection
        .find_one_and_replace(
          bson::doc! { "device_id": update.device_id.clone() },
          update,
          mongodb::options::FindOneAndReplaceOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        )
        .await?;
    }

    Ok(())
  }

  async fn up_auth(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
    let db = mongo.database(&config.mongo.database);
    let origin_collection = db.collection::<OriginDeviceAuthorityRecord>(&config.mongo.collections.device_authorities);
    let target_collection = db.collection::<TargetDeviceAuthorityRecord>(&config.mongo.collections.device_authorities);

    let mut cursor = origin_collection
      .find(bson::doc! { "device_id": { "$exists": 1 } }, None)
      .await?;

    let mut updates = vec![];
    while let Some(n) = cursor.next().await {
      log::debug!("attemping to migrate device authority '{n:?}'");
      let record = n.with_context(|| "unable to deserialize into old")?;

      let updated = TargetDeviceAuthorityRecord {
        device_id: record.device_id,
        authority_model: record.authority_model.map(|model| match model {
          OriginDeviceAuthorityModel::Exclusive(owner) => TargetDeviceAuthorityModel::Exclusive { owner },
          OriginDeviceAuthorityModel::Shared(owner, _) => TargetDeviceAuthorityModel::Shared { owner, guests: vec![] },
          OriginDeviceAuthorityModel::Public(owner, _) => TargetDeviceAuthorityModel::Public { owner, guests: vec![] },
        }),
      };
      updates.push(updated);
    }

    for update in updates {
      log::debug!("applying update '{update:?}'");
      target_collection
        .find_one_and_replace(
          bson::doc! { "device_id": update.device_id.clone() },
          update,
          mongodb::options::FindOneAndReplaceOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        )
        .await?;
    }

    Ok(())
  }

  pub async fn down(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    self.down_auth(config).await?;
    self.down_states(config).await
  }
  pub async fn down_states(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
    let db = mongo.database(&config.mongo.database);
    let origin_collection = db.collection::<OriginDeviceState>(&config.mongo.collections.device_states);
    let target_collection = db.collection::<TargetDeviceState>(&config.mongo.collections.device_states);

    let mut cursor = target_collection
      .find(bson::doc! { "device_id": { "$exists": 1 } }, None)
      .await?;

    let mut updates = vec![];
    while let Some(n) = cursor.next().await {
      log::debug!("attemping to migrate device state '{n:?}'");
      let record = n.with_context(|| "unable to deserialize into old")?;

      let updated = OriginDeviceState {
        updated_at: record.updated_at,
        device_id: record.device_id,
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
      updates.push(updated);
    }

    for update in updates {
      log::debug!("applying update '{update:?}'");
      origin_collection
        .find_one_and_replace(
          bson::doc! { "device_id": update.device_id.clone() },
          update,
          mongodb::options::FindOneAndReplaceOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        )
        .await?;
    }

    Ok(())
  }

  pub async fn down_auth(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    log::debug!("moving from new to old");
    let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
    let db = mongo.database(&config.mongo.database);
    let target_collection = db.collection::<TargetDeviceAuthorityRecord>(&config.mongo.collections.device_authorities);
    let origin_collection = db.collection::<OriginDeviceAuthorityRecord>(&config.mongo.collections.device_authorities);

    let mut cursor = target_collection
      .find(bson::doc! { "device_id": { "$exists": 1 } }, None)
      .await?;

    let mut updates = vec![];
    while let Some(n) = cursor.next().await {
      log::debug!("attemping to migrate");
      let record = n.with_context(|| "unable to deserialize into new")?;

      let updated = OriginDeviceAuthorityRecord {
        device_id: record.device_id,
        authority_model: record.authority_model.map(|model| match model {
          TargetDeviceAuthorityModel::Exclusive { owner } => OriginDeviceAuthorityModel::Exclusive(owner),
          TargetDeviceAuthorityModel::Shared { owner, .. } => OriginDeviceAuthorityModel::Exclusive(owner),
          TargetDeviceAuthorityModel::Public { owner, .. } => OriginDeviceAuthorityModel::Exclusive(owner),
        }),
      };
      updates.push(updated);
    }

    for update in updates {
      log::debug!("updating to '{update:?}'");
      origin_collection
        .find_one_and_replace(
          bson::doc! { "device_id": update.device_id.clone() },
          update,
          mongodb::options::FindOneAndReplaceOptions::builder()
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        )
        .await?;
    }

    Ok(())
  }
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
