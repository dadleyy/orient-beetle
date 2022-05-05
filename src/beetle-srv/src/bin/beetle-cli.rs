use std::io::{Error, ErrorKind, Result};

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
  PushString(String, String),
}

impl Default for CommandLineCommand {
  fn default() -> Self {
    CommandLineCommand::Help
  }
}

#[derive(Default)]
struct CommandLineConfig {
  redis: (String, String, String),
  command: CommandLineCommand,
}

async fn get_connected_page(
  mut stream: &mut async_tls::client::TlsStream<async_std::net::TcpStream>,
  _pagination: Option<u32>,
) -> Result<Vec<beetle::IndexedDevice>> {
  let key_result = kramer::execute(
    &mut stream,
    kramer::Command::Sets::<&str, bool>(kramer::SetCommand::Members(beetle::constants::REGISTRAR_INDEX)),
  )
  .await?;

  match key_result {
    kramer::Response::Array(inner) => {
      let mut items = Vec::with_capacity(inner.len());

      for id in &inner {
        if let kramer::ResponseValue::String(id) = id {
          let item = kramer::execute(
            &mut stream,
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

async fn run(config: CommandLineConfig) -> Result<()> {
  if config.command == CommandLineCommand::Help {
    eprintln!("{}", HELP_TEXT);
    return Ok(());
  }

  let mut stream = beetle::connect(&config.redis.0, &config.redis.1, &config.redis.2).await?;

  match config.command {
    CommandLineCommand::Help => unreachable!(),
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
      let page = get_connected_page(&mut stream, None).await?;

      for dev in &page {
        println!("{}", dev);
      }
    }

    CommandLineCommand::CleanDisconnects => {
      let page = get_connected_page(&mut stream, None).await?;
      let mins = chrono::Utc::now();

      for dev in &page {
        let since = mins.signed_duration_since(*dev.last_seen()).num_seconds();

        if since > 60 {
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

  let redis = std::env::var("REDIS_HOST")
    .ok()
    .zip(std::env::var("REDIS_PORT").ok())
    .zip(std::env::var("REDIS_AUTH").ok())
    .map(|((h, p), a)| (h, p, a));

  let mut config = CommandLineConfig::default();
  let mut args = std::env::args().skip(1);
  let cmd = args.next();

  config.command = match cmd.as_ref().map(|i| i.as_str()) {
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

  if let Some(redis) = redis {
    config.redis = redis;
  }

  async_std::task::block_on(run(config))
}
