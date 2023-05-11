use std::io::{Error, ErrorKind, Result};

/// Takes our mongo configuration as an input, and produces the mongo client provided by the
/// `mongodb` crate.
pub async fn connect_mongo(config: &crate::config::MongoConfiguration) -> Result<mongodb::Client> {
  // Attempt to connect to mongo early.
  let mongo_options = mongodb::options::ClientOptions::parse(&config.url)
    .await
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

  mongodb::Client::with_options(mongo_options)
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))
}
