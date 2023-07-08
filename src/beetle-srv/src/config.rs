//! These configuration schemas are meant to be _shared_ across the different executables products
//! by this project. For exectuable specific configuration, search elsewhere.

use serde::Deserialize;

/// The configuration of our redis connection.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RedisConfiguration {
  /// The host; e.g `redis.upstash.io`
  pub host: String,
  /// The port; e.g `1231`
  pub port: u16,
  /// The password to authenticate with. This is typically the `default` acl role.
  pub auth: Option<String>,
}

/// Google api client credential + endpoint configuration vauoles.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct GoogleConfiguration {
  /// The scopes.
  pub scopes: Vec<String>,
  /// The client id.
  pub client_id: String,
  /// The client secret.
  pub client_secret: String,
  /// The redirect uri.
  pub redirect_uri: String,
}

/// Collection configuration.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MongoCollectionsConfiguration {
  /// The collection name which holds our list of users (which includes their device access).
  pub users: String,

  /// The collection name which holds permission information between devices and users.
  pub device_authorities: String,

  /// The collection to store device information that will be periodically updated as the device
  /// interacts with the server.
  pub device_diagnostics: String,

  /// Storage for scheduled device rendering.
  pub device_schedules: String,

  /// Storage for the history of messages sent to devices.
  pub device_histories: String,

  /// Storage of device states.
  pub device_states: String,
}

/// The mongodb configuration.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MongoConfiguration {
  /// The url that we will pass on to the `mongodb` crate when creating clients.
  pub url: String,
  /// Configures database name we will be using.
  pub database: String,
  /// Configures the collection names inside our database.
  pub collections: MongoCollectionsConfiguration,
}

/// The configuration specific to maintaining a registration of available ids.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct RegistrarConfiguration {
  // TODO: the cli's registar configuration uses these fields, and we may as well.
  /// The auth username that will be given on burn-in to devices.
  pub id_consumer_username: Option<String>,
  /// The auth password that will be given on burn-in to devices.
  pub id_consumer_password: Option<String>,

  /// The minimum amount of ids to maintain. If lower than this, we will refill.
  pub registration_pool_minimum: Option<u8>,

  /// The max amount of devices to update during a iteration of checking device activity.
  pub active_device_chunk_size: u8,

  /// Where to send devices on their initial connection
  pub initial_scannable_addr: String,

  /// The secret used to encrypt vendor api access tokens.
  pub vendor_api_secret: String,
}
