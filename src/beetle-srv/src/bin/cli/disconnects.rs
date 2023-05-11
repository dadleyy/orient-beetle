use std::io;

/// The amount of time to consider as a cutoff amount for devices having sent a "ack" message back
/// to our server. Devices that have not sent a message within this time will be removed.
const MAX_IDLE_TIME_SECONDS: i64 = 60 * 30;

/// Prints connected devices.
pub async fn print_connected(config: &super::CommandLineConfig) -> io::Result<()> {
  let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
  let collection = mongo
    .database(&config.mongo.database)
    .collection::<beetle::types::DeviceDiagnostic>(&config.mongo.collections.device_diagnostics);

  let mut cursor = collection
    .find(None, Some(mongodb::options::FindOptions::builder().limit(50).build()))
    .await
    .map_err(|error| {
      log::warn!("failed mongo query - {error}");
      io::Error::new(io::ErrorKind::Other, format!("{error}"))
    })?;

  let mut count = 0;

  #[allow(clippy::blocks_in_if_conditions)]
  while cursor.advance().await.map_err(|error| {
    log::warn!("unable to advance cursor - {error}");
    io::Error::new(io::ErrorKind::Other, format!("{error}"))
  })? {
    count += 1;
    match cursor.deserialize_current() {
      Ok(device) => {
        println!("- {device}")
      }
      Err(error) => log::warn!("unable to deserialize diagnostic - {error}"),
    }
  }

  if count == 0 {
    println!("no devices found");
  }

  Ok(())
}

/// Removes devices that have not been heard from in a while.
pub async fn clean_disconnects(config: &super::CommandLineConfig) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;

  let collection = mongo
    .database(&config.mongo.database)
    .collection::<beetle::types::DeviceDiagnostic>(&config.mongo.collections.device_diagnostics);

  let cutoff = chrono::Utc::now()
    .checked_sub_signed(chrono::Duration::seconds(MAX_IDLE_TIME_SECONDS))
    .ok_or_else(|| {
      log::warn!("overflow calculation for cutoff");
      io::Error::new(io::ErrorKind::Other, "cutoff time calc overflow")
    })?;

  log::info!("using cutoff value - {cutoff:?} ({})", cutoff.timestamp_millis());
  let cutoff_query = bson::doc! { "last_seen": { "$lt": cutoff.timestamp_millis() } };
  let mut cursor = collection
    .find(
      cutoff_query.clone(),
      Some(mongodb::options::FindOptions::builder().limit(50).build()),
    )
    .await
    .map_err(|error| {
      log::warn!("failed mongo query - {error}");
      io::Error::new(io::ErrorKind::Other, format!("{error}"))
    })?;

  let mut devices = Vec::with_capacity(100);

  #[allow(clippy::blocks_in_if_conditions)]
  while cursor.advance().await.map_err(|error| {
    log::warn!("unable to advance cursor - {error}");
    io::Error::new(io::ErrorKind::Other, format!("{error}"))
  })? {
    let device = cursor.deserialize_current();
    log::info!("found diagnostic {:?}", device);

    if let Ok(d) = device {
      devices.push(d);
    }
  }

  let count = devices.len();

  if count == 0 {
    println!("all devices active within cuttof time!");
    return Ok(());
  }

  println!("- found {count} diagnostics with expired cutoffs, deleting diagnostics");

  if count > 0 {
    let result = collection
      .delete_many(cutoff_query.clone(), None)
      .await
      .map_err(|error| {
        log::warn!("unable to perform delete_many - {error}");
        io::Error::new(io::ErrorKind::Other, format!("{error}"))
      })?;

    log::info!("delete complete - {:?}", result);

    // Cleanup the acl entries of these dead devices.
    kramer::execute(
      &mut stream,
      kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::DelUser(kramer::Arity::Many(
        devices.iter().map(|device| device.id.clone()).collect(),
      ))),
    )
    .await?;
  }

  // Cleanup our redis hash and set.
  for dev in devices {
    println!("  - cleaning up redis resources for device {}", dev.id);

    kramer::execute(
      &mut stream,
      kramer::Command::Sets::<&str, &str>(kramer::SetCommand::Rem(
        beetle::constants::REGISTRAR_INDEX,
        kramer::Arity::One(&dev.id),
      )),
    )
    .await?;

    log::info!("cleaned up up {:?}", dev);
  }

  Ok(())
}
