use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct RedisConfiguration {
  pub(crate) host: String,
  pub(crate) port: u16,
  pub(crate) auth: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct Auth0Configuration {
  pub(crate) client_id: String,
  pub(crate) client_secret: String,
  pub(crate) auth_uri: String,
  pub(crate) redirect_uri: String,
  pub(crate) token_uri: String,
  pub(crate) info_uri: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MongoCollectionsConfiguration {
  pub(crate) users: String,
  pub(crate) devices: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MongoConfiguration {
  pub(crate) url: String,
  pub(crate) database: String,
  pub(crate) collections: MongoCollectionsConfiguration,
}
