#![allow(clippy::missing_docs_in_private_items)]
use anyhow::Context;
use async_std::stream::StreamExt;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "migration:name")]
pub struct Migration {}

impl Migration {
  pub async fn up(&self, config: &crate::cli::CommandLineConfig) -> anyhow::Result<()> {
    log::debug!("moving from old to new");
    let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
    let db = mongo.database(&config.mongo.database);
    let origin_collection = db.collection::<OriginDeviceAuthorityRecord>(&config.mongo.collections.device_authorities);
    let target_collection = db.collection::<TargetDeviceAuthorityRecord>(&config.mongo.collections.device_authorities);

    let mut cursor = origin_collection
      .find(bson::doc! { "device_id": { "$exists": 1 } }, None)
      .await?;

    let mut updates = vec![];
    while let Some(n) = cursor.next().await {
      log::debug!("attemping to migrate");
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
