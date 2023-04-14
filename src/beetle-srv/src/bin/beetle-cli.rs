use clap::Parser;
use serde::Deserialize;
use std::io;

const MAX_IDLE_TIME_SECONDS: i64 = 60 * 30;

#[derive(Parser, Deserialize, PartialEq)]
struct ProvisionCommand {
  user: Option<String>,
  password: Option<String>,
}

#[derive(Parser, Deserialize, PartialEq)]
struct SendMessageCommand {
  id: String,
  message: String,
}

#[derive(PartialEq, clap::Subcommand, Deserialize, Default)]
enum CommandLineCommand {
  #[default]
  PrintConnected,

  /// This command will blow away _all_ current acl entries. At that moment, devices will need to
  /// re-authenticate from a fresh set of available ids.
  InvalidateAcls,

  CleanDisconnects,

  Provision(ProvisionCommand),

  PushString(SendMessageCommand),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
struct RegistrarConfiguration {
  id_consumer_username: Option<String>,
  id_consumer_password: Option<String>,
  // registration_pool_minimum: Option<u8>,
  acl_user_allowlist: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct CommandLineConfig {
  redis: beetle::config::RedisConfiguration,
  mongo: beetle::config::MongoConfiguration,

  registrar: RegistrarConfiguration,
}

#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineOptions {
  #[arg(short = 'c', long)]
  config: String,

  #[command(subcommand)]
  command: CommandLineCommand,
}

fn id_from_acl_entry(entry: &str) -> Option<&str> {
  entry.split(' ').nth(1)
}

async fn run(config: CommandLineConfig, command: CommandLineCommand) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;

  let allowed: std::collections::hash_set::HashSet<String> = std::collections::hash_set::HashSet::from_iter(
    config.registrar.acl_user_allowlist.unwrap_or(vec![]).iter().cloned(),
  );

  match command {
    CommandLineCommand::InvalidateAcls => {
      log::debug!("looking for acl entries to destroy, skipping {allowed:?}");
      let list = kramer::execute(&mut stream, kramer::Command::Acl::<u8, u8>(kramer::AclCommand::List)).await;

      let values = match list {
        Ok(kramer::Response::Array(inner)) => inner,
        _ => return Err(io::Error::new(io::ErrorKind::Other, "")),
      };

      let names = values
        .into_iter()
        .filter_map(|entry| match entry {
          kramer::ResponseValue::String(v) => {
            let id = id_from_acl_entry(&v)?;
            log::trace!("found id {id}");

            if allowed.contains(id) {
              None
            } else {
              Some(id.to_string())
            }
          }
          _ => None,
        })
        .collect::<Vec<String>>();

      if names.is_empty() {
        println!("no matching acl entries to delete");
        return Ok(());
      }

      println!("the following acl entries will be deleted. enter 'y' to continue: {names:?}");
      let mut buffer = String::new();
      io::stdin().read_line(&mut buffer)?;

      if buffer.as_str().trim_end() != "y" {
        println!("aborting.");
        return Ok(());
      }

      // Delete the ACL entries _before_ the queue. This is important so the registrar worker is
      // does not refill acl entries that would be immediate destroyed.
      log::info!("continuing with deletion");
      let command = kramer::Command::<String, &str>::Acl(kramer::AclCommand::DelUser(kramer::Arity::Many(names)));
      kramer::execute(&mut stream, &command).await?;

      log::info!("now clearing off our registration queue");
      let command = kramer::Command::<&str, &str>::Del(kramer::Arity::One(beetle::constants::REGISTRAR_AVAILABLE));
      kramer::execute(&mut stream, &command).await?;
      println!("done.");
    }

    CommandLineCommand::Provision(ProvisionCommand { user, password }) => {
      log::info!("provisioning redis environment with burn-in auth information");

      let password = password.or(config.registrar.id_consumer_password);
      let user = user.or(config.registrar.id_consumer_username);

      match (user, password) {
        (Some(ref user), Some(ref pass)) => {
          let command = kramer::Command::Acl::<&str, &str>(kramer::acl::AclCommand::SetUser(kramer::acl::SetUser {
            name: user,
            password: Some(pass),
            keys: Some(beetle::constants::REGISTRAR_AVAILABLE),
            commands: Some(vec!["lpop", "blpop"]),
          }));

          let result = kramer::execute(&mut stream, &command).await;
          log::debug!("result from {command:?} - {result:?}");
          println!("ok");
        }
        _ => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            "username or pasword missing for provisioning",
          ));
        }
      }
    }

    CommandLineCommand::PushString(SendMessageCommand { id, message }) => {
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
      println!("ok");
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
    }

    CommandLineCommand::CleanDisconnects => {
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
    }
  }

  Ok(())
}

fn main() -> io::Result<()> {
  dotenv::dotenv().ok();
  env_logger::init();

  log::info!("environment + logger ready.");

  let options = CommandLineOptions::parse();
  let contents = std::fs::read_to_string(&options.config)?;
  let config = toml::from_str::<CommandLineConfig>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  async_std::task::block_on(run(config, options.command))
}
