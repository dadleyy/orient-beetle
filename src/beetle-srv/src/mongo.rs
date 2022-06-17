use std::io::{Error, ErrorKind, Result};

pub async fn connect_mongo(config: &crate::config::MongoConfiguration) -> Result<mongodb::Client> {
  // Attempt to connect to mongo early.
  let mongo_options = mongodb::options::ClientOptions::parse(&config.url)
    .await
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

  mongodb::Client::with_options(mongo_options)
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))
}
