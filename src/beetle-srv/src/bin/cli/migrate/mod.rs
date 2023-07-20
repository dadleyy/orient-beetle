#![allow(clippy::missing_docs_in_private_items)]

//! Note: this current implementation of "migrations" only exists to unblock schema refinement
//! during the initial, MVP phase of this project. There are some  benefits to doing it this way
//! where the migration authors have lots of control, but the boilerplate and scalability here is
//! questionable.
//!
//! Effectively, each migration would be implemented as a new variant to our `Migration` enum,
//! where the appropriate "solution" is added to the `up`/`down` method and implemented in the
//! `migrations` module.
//!
//! The migrations define their entire own schema for the before/after and should _not_ use the
//! schema provided by the crate.

use serde::{Deserialize, Serialize};
use std::io;

mod migrations;
mod ops;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "migration:name")]
pub enum Migration {
  M0,
}

impl Migration {
  async fn up(&self, config: &super::CommandLineConfig) -> anyhow::Result<()> {
    match self {
      Self::M0 => migrations::m0::Migration {}.up(config).await,
    }
  }
  async fn down(&self, config: &super::CommandLineConfig) -> anyhow::Result<()> {
    match self {
      Self::M0 => migrations::m0::Migration {}.down(config).await,
    }
  }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, clap::ValueEnum)]
pub enum MigrateOp {
  Up,
  UpForce,
  Down,
  DownForce,
}

#[derive(Debug, Deserialize, Serialize)]
struct MigrationHistory {
  runs: Vec<Migration>,
}

pub async fn run(config: &super::CommandLineConfig, dir: MigrateOp) -> io::Result<()> {
  let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
  let collection = mongo
    .database(&config.mongo.database)
    .collection::<MigrationHistory>(&config.mongo.collections.migrations);

  let full_list = vec![Migration::M0];

  let first = collection
    .find_one(bson::doc! { "runs": { "$exists": 1 } }, None)
    .await
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?
    .unwrap_or(MigrationHistory { runs: vec![] });

  let mut run = std::collections::HashSet::new();

  for alread_run in first.runs {
    let serialized = serde_json::to_string(&alread_run)?;
    run.insert(serialized);
  }

  let mut new_list = vec![];

  for migration in full_list {
    let serialized = serde_json::to_string(&migration)?;
    match (run.contains(&serialized), dir) {
      (_, MigrateOp::UpForce) | (false, MigrateOp::Up) => {
        log::info!("running UP '{serialized}'");
        migration
          .up(config)
          .await
          .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
        new_list.push(migration);
      }
      (_, MigrateOp::DownForce) | (true, MigrateOp::Down) => {
        log::info!("running DOWN '{serialized}'");
        migration
          .down(config)
          .await
          .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
      }
      (true, MigrateOp::Up) => {
        new_list.push(migration);
      }
      _ => continue,
    }
  }

  collection
    .find_one_and_replace(
      bson::doc! { "runs": { "$exists": 1 } },
      MigrationHistory { runs: new_list },
      mongodb::options::FindOneAndReplaceOptions::builder()
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .build(),
    )
    .await
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;

  Ok(())
}
