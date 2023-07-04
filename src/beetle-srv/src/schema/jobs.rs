//! The schema associated with both the renderer and the registrar background jobs.

use serde::{Deserialize, Serialize};

/// The enumerated result of the kinds of "success" a job may have.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum SuccessfulJobResult {
  /// This job was run to completion.
  Terminal,

  /// This job caused other jobs to immediately run.
  Percolated(Vec<String>),
}

/// The enumerated result set of all background jobs.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum JobResult {
  /// The job is currently pending.
  Pending,

  /// A success without any more info.
  Success(SuccessfulJobResult),

  /// A failure with a reason.
  Failure(String),
}
