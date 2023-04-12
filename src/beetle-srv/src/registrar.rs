use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind, Result};

/// If no value is provided in the api, this value will be used as the minimum amount of entries in
/// our pool that we need. If the current amount is less than this, we will generate ids for and
/// store them in the system.
const DEFAULT_POOL_MINIMUM: u8 = 3;

/// The publicly deserializable interface for our registrar worker configuration.
#[derive(Deserialize, Clone)]
pub struct Configuration {
  /// The redis configuration.
  redis: crate::config::RedisConfiguration,
  /// The mongo configuration.
  mongo: crate::config::MongoConfiguration,
}

impl Configuration {
  /// Builds a worker from whatever we were able to serialize from our configuration inputs.
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

/// The container that will be passed around to various registrar internal functions.
pub struct Worker {
  /// The redis configuration.
  redis: crate::config::RedisConfiguration,
  /// The TCP connection we have to our redis host, if we currently have one.
  connection: Option<async_tls::client::TlsStream<async_std::net::TcpStream>>,
  /// The mongo client + configuration
  mongo: (mongodb::Client, crate::config::MongoConfiguration),
}

impl Worker {
  /// The main execution api of our worker. Inside here we perform the responsibilities of
  /// updating our pool if necessary, and marking whatever devices we've heard from as "active".
  pub async fn work(&mut self) -> Result<()> {
    let stream = self.connection.take();

    self.connection = match stream {
      None => {
        log::info!("no previous connection, attempting to connect now");
        crate::redis::connect(&self.redis).await.map(Some)?
      }

      Some(mut inner) => {
        log::trace!("active redis connection, checking pool");
        let amount = fill_pool(
          &mut inner,
          self.redis.registration_pool_minimum.unwrap_or(DEFAULT_POOL_MINIMUM),
        )
        .await?;

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
async fn fill_pool(mut stream: &mut async_tls::client::TlsStream<async_std::net::TcpStream>, min: u8) -> Result<usize> {
  let output = kramer::execute(
    &mut stream,
    kramer::Command::List::<&str, bool>(kramer::ListCommand::Len(crate::constants::REGISTRAR_AVAILABLE)),
  )
  .await?;

  let should_send = match output {
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) if amount < min as i64 => {
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

  if !should_send {
    return Ok(0);
  }

  let ids = (0..min).map(|_| crate::identity::create()).collect::<Vec<String>>();
  let count = ids.len();

  log::info!("creating acl entries for ids");

  for id in &ids {
    let setuser = kramer::acl::SetUser {
      name: id.clone(),
      password: Some(id.clone()),
      commands: Some(vec!["lpop".to_string(), "blpop".to_string()]),
      keys: Some(crate::redis::device_message_queue_id(id)),
    };
    let command = kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::SetUser(setuser));

    if let Err(error) = kramer::execute(&mut stream, &command).await {
      log::warn!("unable to add acl for id '{}' - {error}", id);
    }

    let setuser = kramer::acl::SetUser {
      name: id.clone(),
      password: Some(id.clone()),
      commands: Some(vec!["rpush".to_string()]),
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

/// This type is used by mongo when an existing record is _not_ found.
#[derive(Serialize)]
struct DeviceDiagnosticSetOnInsert {
  /// When inserting, start with the current timestamp .
  #[serde(with = "chrono::serde::ts_milliseconds")]
  first_seen: chrono::DateTime<chrono::Utc>,
}

/// If mongo already has an entry for this device, this type will be used for the "update" portion
/// of our request.
#[derive(Serialize)]
struct DeviceDiagnosticUpsert<'a> {
  /// The id of our device.
  id: &'a String,

  /// The timestamp we should now be updating.
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
      .ok_or_else(|| Error::new(ErrorKind::Other, "upsert failed"))?;

    // Store the device identity in a set; this will allow us to iterate over the list of
    // active ids more easily later.
    let setter = kramer::Command::Sets(kramer::SetCommand::Add(
      crate::constants::REGISTRAR_INDEX,
      kramer::Arity::One(id.as_str()),
    ));
    kramer::execute(&mut stream, setter).await?;

    log::info!("updated device '{}' diagnostics", device_diagnostic.id);
  }

  Ok(0usize)
}
