use serde::Deserialize;

/// Newrelic configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct NewRelic {
  /// The newrelic account id.
  pub account_id: String,

  /// The newrelic license key.
  pub api_key: String,
}
