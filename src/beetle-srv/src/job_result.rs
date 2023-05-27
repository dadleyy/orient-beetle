use serde::{Deserialize, Serialize};

/// The enumerated result set of all background jobs.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum JobResult {
  /// The job is currently pending.
  Pending,

  /// A success without any more info.
  Success,

  /// A failure with a reason.
  Failure(String),
}
