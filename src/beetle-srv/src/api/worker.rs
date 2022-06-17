use std::io::{Error, ErrorKind, Result};

#[derive(Clone)]
pub struct Worker {
  pub(super) web_configuration: super::WebConfiguration,
  pub(super) redis_configuration: crate::config::RedisConfiguration,
  pub(super) auth0_configuration: crate::config::Auth0Configuration,
  pub(super) mongo: (mongodb::Client, crate::config::MongoConfiguration),
}

impl Worker {
  pub(super) async fn redis(&self) -> Result<async_tls::client::TlsStream<async_std::net::TcpStream>> {
    crate::redis::connect(&self.redis_configuration).await
  }

  pub(super) async fn command<S, V>(&self, command: &kramer::Command<S, V>) -> Result<kramer::Response>
  where
    S: std::fmt::Display,
    V: std::fmt::Display,
  {
    let mut stream = self.redis().await?;
    kramer::execute(&mut stream, command).await
  }

  pub(super) async fn request_authority(&self, request: &tide::Request<Self>) -> Result<Option<crate::types::User>> {
    let oid = request
      .cookie(&self.web_configuration.session_cookie)
      .and_then(|cook| super::claims::Claims::decode(cook.value(), &self.web_configuration.session_secret).ok())
      .map(|claims| claims.oid)
      .unwrap_or_default();

    if oid.len() == 0 {
      return Ok(None);
    }

    log::debug!("attempting to identify via {:?}", oid);
    let users = self.users_collection()?;
    let query = bson::doc! { "oid": oid.clone() };

    users.find_one(query, None).await.map_err(|error| {
      log::warn!("unable to create new player - {:?}", error);
      Error::new(ErrorKind::Other, format!("bad-query - {error}"))
    })
  }

  pub(super) fn device_diagnostic_collection(&self) -> Result<mongodb::Collection<crate::types::DeviceDiagnostic>> {
    Ok(
      self
        .mongo
        .0
        .database(&self.mongo.1.database)
        .collection(&self.mongo.1.collections.device_diagnostics),
    )
  }

  pub(super) fn users_collection(&self) -> Result<mongodb::Collection<crate::types::User>> {
    Ok(
      self
        .mongo
        .0
        .database(&self.mongo.1.database)
        .collection(&self.mongo.1.collections.users),
    )
  }
}
