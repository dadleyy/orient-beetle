use serde::Deserialize;
use std::io::{Error, ErrorKind, Result};

const MAX_IDLE_TIME_SECONDS: i64 = 60 * 30;
const HELP_TEXT: &'static str = r#"beetle-cli admin interface

usage:
    beetle-cli help
    beetle-cli printall
    beetle-cli write <id> <message>
"#;

#[derive(PartialEq)]
enum CommandLineCommand {
  Help,
  PrintConnected,
  CleanDisconnects,
  Provision(String, String),
  PushString(String, String),
}

impl Default for CommandLineCommand {
  fn default() -> Self {
    CommandLineCommand::Help
  }
}

#[derive(Deserialize)]
struct RegistrarConfiguration {
  id_consumer_username: Option<String>,
  id_consumer_password: Option<String>,
}

#[derive(Deserialize)]
struct CommandLineConfig {
  redis: beetle::config::RedisConfiguration,
  mongo: beetle::config::MongoConfiguration,

  registrar: RegistrarConfiguration,
}

async fn run(config: CommandLineConfig, command: CommandLineCommand) -> Result<()> {
  if command == CommandLineCommand::Help {
    eprintln!("{}", HELP_TEXT);
    return Ok(());
  }

  let mut stream = beetle::redis::connect(&config.redis).await?;
  let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;

  match command {
    CommandLineCommand::Help => unreachable!(),

    CommandLineCommand::Provision(user, password) => {
      log::info!("provisioning redis environment with device auth information");
      let command = kramer::Command::Acl::<&str, &str>(kramer::acl::AclCommand::SetUser(kramer::acl::SetUser {
        name: &user,
        password: Some(&password),
        keys: Some(beetle::constants::REGISTRAR_AVAILABLE),
        commands: Some("LPOP"),
      }));
      let result = kramer::execute(&mut stream, &command).await;
      log::info!("result - {result:?}");
    }

    CommandLineCommand::PushString(id, message) => {
      log::debug!("writing '{}' to '{}'", message, id);

      let result = kramer::execute(
        &mut stream,
        kramer::Command::List(kramer::ListCommand::Push(
          (kramer::Side::Left, kramer::Insertion::Always),
          beetle::redis::device_message_queue_id(id),
          kramer::Arity::One(message),
        )),
      )
      .await?;

      log::info!("message result - {result:?}");
    }

    CommandLineCommand::PrintConnected => {
      let collection = mongo
        .database(&config.mongo.database)
        .collection::<beetle::types::DeviceDiagnostic>(&config.mongo.collections.device_diagnostics);

      let mut cursor = collection
        .find(None, Some(mongodb::options::FindOptions::builder().limit(50).build()))
        .await
        .map_err(|error| {
          log::warn!("failed mongo query - {error}");
          Error::new(ErrorKind::Other, format!("{error}"))
        })?;

      while cursor.advance().await.map_err(|error| {
        log::warn!("unable to advance cursor - {error}");
        Error::new(ErrorKind::Other, format!("{error}"))
      })? {
        match cursor.deserialize_current() {
          Ok(device) => {
            println!(
              "- {}. last seen {:?}. first seen {:?}",
              device.id, device.last_seen, device.first_seen
            )
          }
          Err(error) => log::warn!("unable to deserialize diagnostic - {error}"),
        }
      }
    }

    CommandLineCommand::CleanDisconnects => {
      let collection = mongo
        .database(&config.mongo.database)
        .collection::<beetle::types::DeviceDiagnostic>(&config.mongo.collections.device_diagnostics);

      let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::seconds(MAX_IDLE_TIME_SECONDS))
        .ok_or_else(|| {
          log::warn!("overflow calculation for cutoff");
          Error::new(ErrorKind::Other, "cutoff time calc overflow")
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
          Error::new(ErrorKind::Other, format!("{error}"))
        })?;

      let mut devices = Vec::with_capacity(100);

      while cursor.advance().await.map_err(|error| {
        log::warn!("unable to advance cursor - {error}");
        Error::new(ErrorKind::Other, format!("{error}"))
      })? {
        let device = cursor.deserialize_current();
        log::info!("found diagnostic {:?}", device);

        if let Ok(d) = device {
          devices.push(d);
        }
      }

      let count = devices.len();

      log::info!("found {count} diagnostics");

      if count > 0 {
        let result = collection
          .delete_many(cutoff_query.clone(), None)
          .await
          .map_err(|error| {
            log::warn!("unable to perform delete_many - {error}");
            Error::new(ErrorKind::Other, format!("{error}"))
          })?;

        log::info!("delete complete - {:?}", result);
      }

      // Cleanup the acl entries of these dead devices.
      kramer::execute(
        &mut stream,
        kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::DelUser(kramer::Arity::Many(
          devices.iter().map(|device| device.id.clone()).collect(),
        ))),
      )
      .await?;

      // Cleanup our redis hash and set.
      for dev in devices {
        kramer::execute(
          &mut stream,
          kramer::Command::Hashes::<&str, &str>(kramer::HashCommand::Del(
            beetle::constants::REGISTRAR_ACTIVE,
            kramer::Arity::One(&dev.id),
          )),
        )
        .await?;

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
    }
  }

  Ok(())
}

fn main() -> Result<()> {
  dotenv::dotenv().ok();
  env_logger::init();

  log::info!("environment + logger ready.");

  let contents = std::fs::read_to_string("env.toml")?;

  let config = toml::from_str::<CommandLineConfig>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    Error::new(ErrorKind::Other, "bad-config")
  })?;

  let mut args = std::env::args().skip(1);
  let cmd = args.next();

  let command = match cmd.as_ref().map(|i| i.as_str()) {
    Some("provision") => CommandLineCommand::Provision(
      args
        .next()
        .or_else(|| {
          log::info!("no username provided, falling back to 'env.toml'");
          config.registrar.id_consumer_username.clone()
        })
        .ok_or_else(|| Error::new(ErrorKind::Other, "must provide username to provision command"))?,
      args
        .next()
        .or_else(|| {
          log::info!("no password provided, falling back to 'env.toml'");
          config.registrar.id_consumer_password.clone()
        })
        .ok_or_else(|| Error::new(ErrorKind::Other, "must provide password to provision command"))?,
    ),
    Some("printall") => CommandLineCommand::PrintConnected,
    Some("cleanup") => CommandLineCommand::CleanDisconnects,
    Some("write") => {
      let (id, message) = args
        .next()
        .zip(args.next())
        .ok_or_else(|| Error::new(ErrorKind::Other, "invalid"))?;

      log::info!("write command");
      CommandLineCommand::PushString(id, message)
    }
    None | Some("help") => CommandLineCommand::Help,
    Some(other) => {
      eprintln!("unrecognized command '{}'", other);
      CommandLineCommand::Help
    }
  };

  async_std::task::block_on(run(config, command))
}
