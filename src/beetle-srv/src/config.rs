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
  pub auth: String,
}

/// Auth0 api client credential + endpoint configuration vauoles.
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct Auth0Configuration {
  /// Auth0 api client credential.
  pub(crate) client_id: String,
  /// Auth0 api client credential.
  pub(crate) client_secret: String,
  /// The uri association with this specific organization/auth0 application.
  pub(crate) auth_uri: String,
  /// The uri to pass along as our redirect uri as part of the Oauth flow.
  pub(crate) redirect_uri: String,
  /// The api to use for creating tokens.
  pub(crate) token_uri: String,
  /// The api uri to use for fetching user information; this is to verify things, less so to store
  /// additional data.
  pub(crate) info_uri: String,
}

/// Collection configuration.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MongoCollectionsConfiguration {
  /// The collection name which holds our list of users (which includes their device access).
  pub users: String,
  /// The collection to store device information that will be periodically updated as the device
  /// interacts with the server.
  pub device_diagnostics: String,
}

/// The mongodb configuration.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct MongoConfiguration {
  /// The url that we will pass on to the `mongodb` crate when creating clients.
  pub(crate) url: String,
  /// Configures database name we will be using.
  pub database: String,
  /// Configures the collection names inside our database.
  pub collections: MongoCollectionsConfiguration,
}
