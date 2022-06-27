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
struct CommandLineConfig {
  redis: beetle::config::RedisConfiguration,
  mongo: beetle::config::MongoConfiguration,
}

async fn get_connected_page(
  mut connections: (
    &mut async_tls::client::TlsStream<async_std::net::TcpStream>,
    (&mut mongodb::Client, &beetle::config::MongoConfiguration),
  ),
  _pagination: Option<u32>,
) -> Result<Vec<beetle::IndexedDevice>> {
  log::info!("fetching page");

  let key_result = kramer::execute(
    &mut connections.0,
    kramer::Command::Sets::<&str, bool>(kramer::SetCommand::Members(beetle::constants::REGISTRAR_INDEX)),
  )
  .await?;

  log::info!("key result - {key_result:?}");

  match key_result {
    kramer::Response::Array(inner) => {
      let mut items = Vec::with_capacity(inner.len());

      for id in &inner {
        if let kramer::ResponseValue::String(id) = id {
          let item = kramer::execute(
            &mut connections.0,
            kramer::Command::Hashes::<&str, &str>(kramer::HashCommand::Get(
              beetle::constants::REGISTRAR_ACTIVE,
              Some(kramer::Arity::One(&id)),
            )),
          )
          .await?;
          log::info!("found device info - {:?}", item);
          items.push((id, item));

          continue;
        }

        log::warn!("unrecognized item - {id:?}");
      }

      let items = items
        .into_iter()
        .filter_map(|(id, res)| match res {
          kramer::Response::Item(kramer::ResponseValue::String(i)) => Some((id.clone(), i)),
          other => {
            log::warn!("individual item problem - {other:?}");
            None
          }
        })
        .filter_map(|(i, d)| beetle::IndexedDevice::from_redis(&i, &d))
        .collect();

      Ok(items)
    }
    other => {
      log::warn!("unrecognized active device list - {other:?}");
      Err(Error::new(ErrorKind::Other, "unexpected response"))
    }
  }
}

async fn run(config: CommandLineConfig, command: CommandLineCommand) -> Result<()> {
  if command == CommandLineCommand::Help {
    eprintln!("{}", HELP_TEXT);
    return Ok(());
  }

  let mut stream = beetle::redis::connect(&config.redis).await?;
  let mut mongo = beetle::mongo::connect_mongo(&config.mongo).await?;

  match command {
    CommandLineCommand::Help => unreachable!(),

    CommandLineCommand::Provision(user, password) => {
      log::info!("provisioning redis environment with device auth information");
      let command = kramer::Command::Acl::<&str, &str>(kramer::acl::AclCommand::SetUser(kramer::acl::SetUser {
        name: &user,
        password: Some(&password),
        keys: Some(beetle::constants::REGISTRAR_AVAILABLE),
        commands: Some("blpop"),
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
          format!("ob:{}", id),
          kramer::Arity::One(message),
        )),
      )
      .await?;

      log::info!("message result - {result:?}");
    }
    CommandLineCommand::PrintConnected => {
      let page = get_connected_page((&mut stream, (&mut mongo, &config.mongo)), None).await?;

      for dev in &page {
        println!("{}", dev);
      }
    }

    CommandLineCommand::CleanDisconnects => {
      let page = get_connected_page((&mut stream, (&mut mongo, &config.mongo)), None).await?;
      let mins = chrono::Utc::now();

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

      let mut count = 0u32;
      while cursor.advance().await.map_err(|error| {
        log::warn!("unable to advance cursor - {error}");
        Error::new(ErrorKind::Other, format!("{error}"))
      })? {
        count += 1;
        log::info!("found diagnostic {:?}", cursor.deserialize_current());
      }

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

      for dev in &page {
        let since = mins.signed_duration_since(*dev.last_seen()).num_seconds();

        if since > MAX_IDLE_TIME_SECONDS {
          kramer::execute(
            &mut stream,
            kramer::Command::Hashes::<&str, &str>(kramer::HashCommand::Del(
              beetle::constants::REGISTRAR_ACTIVE,
              kramer::Arity::One(dev.id()),
            )),
          )
          .await?;

          kramer::execute(
            &mut stream,
            kramer::Command::Sets::<&str, &str>(kramer::SetCommand::Rem(
              beetle::constants::REGISTRAR_INDEX,
              kramer::Arity::One(dev.id()),
            )),
          )
          .await?;

          log::info!("cleaned up up {}", dev);
        }
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
        .ok_or_else(|| Error::new(ErrorKind::Other, "must provide username to provision command"))?,
      args
        .next()
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
