//! This module contains the majortiy of all "background" operations that are performed outside of
//! the context of a web request. This includes, but is not limited to, checking for active devices
//! and updating `last_seen` timestamps, polling + executing jobs.
//!
//! TODO(s):
//!     - The whole layout of the module could use some fine tuning; there is a `jobs` submodule
//!       but there are nice logical places for the jobs themselves anyways (e.g `ownership` vs.
//!       `rename`, vs etc...)
//!     - The ownership job types could probably be consolidated into a single enum. The ownership
//!       "change" vs ownership "request" doesnt make sense.

use serde::Deserialize;
use std::io;

/// The ownership model defines types + functions associated with managing a devices ownership.
pub(crate) mod ownership;

/// This module defines functionality associated with managing the acl pool.
mod pool;

/// Jobs associated with users.
mod users;

/// Things that happen on a schedule.
mod schedule;

/// This module defines functionality associated with managing the acl pool.
mod diagnostics;

/// Defines the rename device job.
mod rename;

/// Defines rules for what can be done to devices.
mod access;
pub use access::{user_access, AccessLevel};

/// Just a place to put the types generally associated with background work.
pub(crate) mod jobs;
pub use jobs::{RegistrarJob, RegistrarJobKind};

mod worker;
pub use worker::Worker;

/// The publicly deserializable interface for our registrar worker configuration. This is used by
/// the registrar cli application as a way to bundle all of the various configuration schemas into
/// one that we can use in our `toml` config files.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Configuration {
  /// The redis configuration.
  pub redis: crate::config::RedisConfiguration,
  /// The mongo configuration.
  pub mongo: crate::config::MongoConfiguration,
  /// The google configuration.
  pub google: crate::config::GoogleConfiguration,
  /// The configuration specific to maintaining a registration of available ids.
  pub registrar: crate::config::RegistrarConfiguration,
}

impl Configuration {
  /// Builds a worker from whatever we were able to serialize from our configuration inputs.
  pub async fn worker(self) -> io::Result<Worker> {
    let mongo = worker::WorkerMongo::new(&self.mongo.url, self.mongo.clone()).await?;

    Ok(Worker {
      config: self.registrar,
      redis: self.redis,
      google: self.google,
      connection: None,
      mongo,
    })
  }
}
