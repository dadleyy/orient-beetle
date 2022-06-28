use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind, Result};

#[derive(Deserialize, Clone)]
pub struct Configuration {
  redis: crate::config::RedisConfiguration,
  mongo: crate::config::MongoConfiguration,
}

impl Configuration {
  pub async fn worker(self) -> Result<Worker> {
    let mongo_options = mongodb::options::ClientOptions::parse(&self.mongo.url)
      .await
      .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    let mongo = mongodb::Client::with_options(mongo_options)
      .map_err(|error| Error::new(ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    Ok(Worker {
      redis: self.redis,
      connection: None,
      mongo: (mongo, self.mongo.clone()),
    })
  }
}

pub struct Worker {
  redis: crate::config::RedisConfiguration,
  connection: Option<async_tls::client::TlsStream<async_std::net::TcpStream>>,
  mongo: (mongodb::Client, crate::config::MongoConfiguration),
}

impl Worker {
  pub async fn work(&mut self) -> Result<()> {
    let stream = self.connection.take();

    self.connection = match stream {
      None => {
        log::info!("no previous connection, attempting to connect now");
        crate::redis::connect(&self.redis).await.map(Some)?
      }

      Some(mut inner) => {
        log::trace!("active redis connection, checking pool");
        let amount = fill_pool(&mut inner).await?;

        if amount > 0 {
          log::info!("filled pool with '{}' new ids", amount)
        }

        log::trace!("checking active device queue");
        mark_active(&mut inner, &mut self.mongo.0, &self.mongo.1).await?;

        Some(inner)
      }
    };

    Ok(())
  }
}

/// The main thing our worker will be responsible for is to count the amount of available ids
/// in our pool that devices will pull down to identify themselves. If that amount reaches a
/// quantity below a specific threshold, fill it back up.
async fn fill_pool(mut stream: &mut async_tls::client::TlsStream<async_std::net::TcpStream>) -> Result<usize> {
  log::debug!("checking pool length");

  let output = kramer::execute(
    &mut stream,
    kramer::Command::List::<&str, bool>(kramer::ListCommand::Len(crate::constants::REGISTRAR_AVAILABLE)),
  )
  .await?;

  let should_send = match output {
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) if amount < 3 => {
      log::debug!("not enough ids, populating");
      true
    }
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) => {
      log::trace!("nothing to do, plenty of ids ('{amount}')");
      false
    }
    other => {
      log::warn!("unexpected response from count: {:?}", other);
      false
    }
  };

  if should_send == false {
    return Ok(0);
  }

  let ids = (0..3).map(|_| crate::identity::create()).collect::<Vec<String>>();
  let count = ids.len();

  log::info!("creating acl entries for ids");

  for id in &ids {
    let setuser = kramer::acl::SetUser {
      name: id.clone(),
      password: Some(id.clone()),
      commands: Some("lpop".to_string()),
      keys: Some(crate::redis::device_message_queue_id(id)),
    };
    let command = kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::SetUser(setuser));

    if let Err(error) = kramer::execute(&mut stream, &command).await {
      log::warn!("unable to add acl for id '{}' - {error}", id);
    }

    let setuser = kramer::acl::SetUser {
      name: id.clone(),
      password: Some(id.clone()),
      commands: Some("rpush".to_string()),
      keys: Some(crate::constants::REGISTRAR_INCOMING.to_string()),
    };
    let command = kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::SetUser(setuser));

    if let Err(error) = kramer::execute(&mut stream, &command).await {
      log::warn!("unable to add acl for id '{}' - {error}", id);
    }
  }

  log::info!("populating ids - {:?}", ids);

  let insertion = kramer::execute(
    &mut stream,
    kramer::Command::List(kramer::ListCommand::Push(
      (kramer::Side::Left, kramer::Insertion::Always),
      crate::constants::REGISTRAR_AVAILABLE,
      kramer::Arity::Many(ids),
    )),
  )
  .await?;

  log::debug!("insertion result - {:?}", insertion);

  Ok(count)
}

#[derive(Serialize)]
struct DeviceDiagnosticSetOnInsert {
  #[serde(with = "chrono::serde::ts_milliseconds")]
  first_seen: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
struct DeviceDiagnosticUpsert<'a> {
  id: &'a String,
  #[serde(with = "chrono::serde::ts_milliseconds")]
  last_seen: chrono::DateTime<chrono::Utc>,
}

/// The second main function of our registrar is to keep our server informed of the active devices
/// by pulling off a queue that is pushed to by devices during regular operating procedure. With an
/// id pulled from the queue, we will store:
///
/// 1. the current timestamp in a hash of `<id> -> timestamp`
/// 2. the id we received in a `Set` for easy indexing.
async fn mark_active<R>(
  mut stream: &mut R,
  db: &mut mongodb::Client,
  dbc: &crate::config::MongoConfiguration,
) -> Result<usize>
where
  R: async_std::io::Read + async_std::io::Write + Unpin,
{
  let taken = kramer::execute(
    &mut stream,
    kramer::Command::List::<&str, bool>(kramer::ListCommand::Pop(
      kramer::Side::Left,
      crate::constants::REGISTRAR_INCOMING,
      None,
    )),
  )
  .await?;

  if let kramer::Response::Item(kramer::ResponseValue::String(id)) = taken {
    log::debug!("device '{}' submitted registration", id);
    let collection = db
      .database(&dbc.database)
      .collection::<crate::types::DeviceDiagnostic>(&dbc.collections.device_diagnostics);

    // Attempt to update the diagnostic information in mongo. We only really want to set `last_seen`
    // on every message; to set `first_seen`, we'll take advantage of mongo's `$setOnInsert`
    // operation.
    let device_diagnostic = collection
      .find_one_and_update(
        bson::doc! { "id": &id },
        bson::to_document(&DeviceDiagnosticUpsert {
          id: &id,
          last_seen: chrono::Utc::now(),
        })
        .and_then(|left| {
          bson::to_document(&DeviceDiagnosticSetOnInsert {
            first_seen: chrono::Utc::now(),
          })
          .map(|right| (left, right))
        })
        .map(|(l, r)| {
          bson::doc! {
            "$set": l,
            "$setOnInsert": r
          }
        })
        .map_err(|error| {
          log::warn!("unable to build upsert doc - {error}");
          Error::new(ErrorKind::Other, format!("{error}"))
        })?,
        Some(
          mongodb::options::FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        ),
      )
      .await
      .map_err(|error| {
        log::warn!("unable to upsert device diagnostic - {error}");
        Error::new(ErrorKind::Other, format!("{error}"))
      })?
      .ok_or_else(|| Error::new(ErrorKind::Other, format!("upsert failed")))?;

    log::info!("updated device '{}' diagnostics", device_diagnostic.id);

    // Store the current timestamp in a hash whose keys are the identity of our devices.
    let activation = kramer::execute(
      &mut stream,
      kramer::Command::Hashes(kramer::HashCommand::Set(
        crate::constants::REGISTRAR_ACTIVE,
        kramer::Arity::One((id.as_str(), chrono::Utc::now().to_rfc3339())),
        kramer::Insertion::Always,
      )),
    )
    .await?;

    log::trace!("device activation - {:?}", activation);

    // Store the device identity in a set; this will allow us to iterate over the list of
    // active ids more easily later.
    let setter = kramer::Command::Sets(kramer::SetCommand::Add(
      crate::constants::REGISTRAR_INDEX,
      kramer::Arity::One(id.as_str()),
    ));
    let activation = kramer::execute(&mut stream, setter).await?;

    log::trace!("device indexing - {:?}", activation);
  }

  Ok(0usize)
}
