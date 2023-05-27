#![deny(unsafe_code)]

//! This executable is meant to provide support for local development; this application mirrors the
//! platformio firmware running on the embedded device - images will be pulled + downloaded to the
//! local filesystem.

use clap::Parser;
use std::io;

#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineArguments {
  #[clap(short, long, default_value = "env.toml")]
  config: String,

  #[clap(short, long, default_value = ".storage")]
  storage: String,
}

#[derive(Debug, PartialEq)]
enum BulkStringLocation {
  Sizing(usize),
  Reading(usize, String, bool),
}

#[derive(Debug, Default, PartialEq)]
enum MessageState {
  #[default]
  Initial,

  ArraySize(usize, bool),

  BulkString(BulkStringLocation, Option<(Vec<String>, usize)>),

  Error(String),
}

impl MessageState {
  fn take(self, token: char) -> Self {
    match (self, token) {
      (Self::Initial, '*') => Self::ArraySize(0, false),
      (Self::Initial, '$') => Self::BulkString(BulkStringLocation::Sizing(0), None),

      (Self::ArraySize(accumulator, _), '\r') => Self::ArraySize(accumulator, true),
      (Self::ArraySize(accumulator, _), '\n') => {
        Self::BulkString(BulkStringLocation::Sizing(0), Some((Vec::new(), accumulator)))
      }

      (Self::ArraySize(accumulator, false), token) => Self::ArraySize(
        (accumulator * 10) + token.to_digit(10).unwrap_or_default() as usize,
        false,
      ),

      (Self::BulkString(BulkStringLocation::Sizing(_), Some((vec, array_size))), '$') => {
        Self::BulkString(BulkStringLocation::Sizing(0), Some((vec, array_size)))
      }
      (Self::BulkString(BulkStringLocation::Sizing(size), Some((vec, array_size))), '\r') => {
        Self::BulkString(BulkStringLocation::Sizing(size), Some((vec, array_size)))
      }

      // Terminate Bulk String Sizing
      (Self::BulkString(BulkStringLocation::Sizing(size), Some((vec, array_size))), '\n') => Self::BulkString(
        BulkStringLocation::Reading(size, String::new(), false),
        Some((vec, array_size)),
      ),

      (Self::BulkString(BulkStringLocation::Sizing(size), Some((vec, array_size))), token) => {
        let new_size = (size * 10) + token.to_digit(10).unwrap_or_default() as usize;
        Self::BulkString(BulkStringLocation::Sizing(new_size), Some((vec, array_size)))
      }

      // Start Bulk String Terminate
      (Self::BulkString(BulkStringLocation::Reading(size, mut buffer, false), Some((vec, array_size))), '\r') => {
        buffer.push(token);
        Self::BulkString(BulkStringLocation::Reading(size, buffer, true), Some((vec, array_size)))
      }

      // Start Bulk String Terminate
      (Self::BulkString(BulkStringLocation::Reading(_, mut buffer, true), Some((mut vec, array_size))), '\n') => {
        vec.push(buffer.drain(0..buffer.len() - 1).collect());
        Self::BulkString(BulkStringLocation::Sizing(0), Some((vec, array_size)))
      }

      (Self::BulkString(BulkStringLocation::Reading(size, mut buffer, _), Some((vec, array_size))), token) => {
        buffer.push(token);
        Self::BulkString(
          BulkStringLocation::Reading(size, buffer, false),
          Some((vec, array_size)),
        )
      }

      (Self::Initial, token) => Self::Error(format!("Invalid starting token '{token}'")),
      (Self::Error(e), _) => Self::Error(e),
      (_, token) => Self::Error(format!("Invalid token '{token}'")),
    }
  }
}

fn save_image(args: &CommandLineArguments, image_buffer: Vec<u8>) -> io::Result<()> {
  log::debug!("attempting to save image buffer of {} byte(s)", image_buffer.len());

  let loaded_image = image::guess_format(image_buffer.as_slice())
    .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("{error}")))?;

  if !matches!(loaded_image, image::ImageFormat::Png) {
    return Err(io::Error::new(
      io::ErrorKind::Other,
      format!("invalid image format - {loaded_image:?}"),
    ));
  }

  log::info!("loaded image - {loaded_image:?}");

  let uuid = uuid::Uuid::new_v4().to_string();
  let mut image_path = std::path::PathBuf::new();
  image_path.push(&args.storage);
  image_path.push(format!("{uuid}.png"));
  let mut file = std::fs::File::create(&image_path)?;
  log::info!("saving to '{:?}'", image_path);
  std::io::Write::write_all(&mut file, image_buffer.as_slice())?;
  Ok(())
}

async fn get_device_id(
  args: &CommandLineArguments,
  config: &beetle::registrar::Configuration,
  mut connection: &mut beetle::redis::RedisConnection,
) -> io::Result<String> {
  let mut id_storage_path = std::path::PathBuf::from(&args.storage);
  id_storage_path.push(".device_id");

  let (id_user, id_password) = config
    .registrar
    .id_consumer_username
    .as_ref()
    .zip(config.registrar.id_consumer_password.as_ref())
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        "Configuration is missing registrar burn-in credentials",
      )
    })?;

  match async_std::fs::metadata(&id_storage_path).await {
    Err(error) if error.kind() == io::ErrorKind::NotFound => {
      let burnin_auth_response = match kramer::execute(
        &mut connection,
        kramer::Command::<&str, &str>::Auth(kramer::AuthCredentials::User((id_user.as_str(), id_password.as_str()))),
      )
      .await
      {
        Ok(kramer::Response::Item(kramer::ResponseValue::String(inner))) if inner == "OK" => inner,
        other => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("unable to authenticate with redis - {other:?}"),
          ))
        }
      };

      log::info!("initial handshake completed {burnin_auth_response:?}, taking a device id");

      let mock_device_id = match kramer::execute(
        &mut connection,
        kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Pop(
          kramer::Side::Left,
          beetle::constants::REGISTRAR_AVAILABLE,
          None,
        )),
      )
      .await
      {
        Ok(kramer::Response::Item(kramer::ResponseValue::String(id))) => id,
        other => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("unable to pull id - {other:?}"),
          ))
        }
      };

      log::info!("device id taken - {mock_device_id:?}");

      match kramer::execute(
        &mut connection,
        kramer::Command::<&str, &str>::Auth(kramer::AuthCredentials::User((&mock_device_id, &mock_device_id))),
      )
      .await
      {
        Ok(kramer::Response::Item(kramer::ResponseValue::String(inner))) if inner == "OK" => (),
        other => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("unable to authenticate with redis - {other:?}"),
          ))
        }
      }

      log::info!("preparing '{}' for device id storage", args.storage);
      async_std::fs::create_dir_all(&args.storage).await?;
      async_std::fs::write(&id_storage_path, &mock_device_id).await?;

      Ok(mock_device_id)
    }

    Ok(meta) if meta.is_file() => {
      log::info!("found existing device id at '{:?}'", id_storage_path);
      let loaded_id = async_std::fs::read_to_string(&id_storage_path).await?;
      log::info!("loaded device id - '{loaded_id}'");

      Ok(loaded_id)
    }
    other @ Ok(_) | other @ Err(_) => Err(io::Error::new(
      io::ErrorKind::Other,
      format!("unable to handle device id storage lookup - {other:?}"),
    )),
  }
}

async fn run(args: CommandLineArguments) -> io::Result<()> {
  let contents = async_std::fs::read_to_string(&args.config).await?;
  let config = toml::from_str::<beetle::registrar::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  log::info!("mock starting with config - '{config:?}'");

  let mut connection = beetle::redis::connect(&config.redis).await.map_err(|error| {
    io::Error::new(
      io::ErrorKind::Other,
      format!("failed redis connection '{:?}' - {error}", config.redis),
    )
  })?;

  let mock_device_id = get_device_id(&args, &config, &mut connection).await?;

  let mut interval = async_std::stream::interval(std::time::Duration::from_millis(500));

  loop {
    log::info!("mock starting image queue pop");
    async_std::stream::StreamExt::next(&mut interval).await;

    async_std::io::WriteExt::write_all(
      &mut connection,
      format!(
        "{}",
        kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Pop(
          kramer::Side::Left,
          beetle::redis::device_message_queue_id(&mock_device_id).as_str(),
          Some((None, 5)),
        ))
      )
      .as_bytes(),
    )
    .await?;

    let mut image_buffer: Vec<u8> = Vec::with_capacity(1024 * 10);
    let mut parser = MessageState::default();

    log::info!("pop written, waiting for response");

    // TODO: this does not seem very structurally sound; the goal is to read from the redis
    // connection, attempting to parse our pop messages as a payload of image data.
    'response_read: loop {
      let mut frame_buffer = [0u8; 1024 * 8];

      match async_std::io::timeout(
        std::time::Duration::from_secs(6),
        async_std::io::ReadExt::read(&mut connection, &mut frame_buffer),
      )
      .await
      {
        Ok(amount) => {
          log::info!("has {amount} bytes");

          if amount == 5 && matches!(std::str::from_utf8(&frame_buffer[0..amount]), Ok("*-1\r\n")) {
            log::info!("empty read from redis, moving on immediately");
            break 'response_read;
          }

          // Try to parse _something_ - this will normally be the `*2\r\n...` bit that contains our
          // array size followed by two entries for the key + actual image data.
          let (message_header, header_size) = match std::str::from_utf8(&frame_buffer) {
            Ok(inner) => {
              log::info!("parsed whole buffer as utf-8 - '{inner}'");
              (Ok(inner), amount)
            }
            Err(error) if error.valid_up_to() > 0 => {
              log::warn!("parially read buffer as utf8 - {error:?}");
              let header_size = error.valid_up_to();
              (std::str::from_utf8(&frame_buffer[0..header_size]), header_size)
            }
            Err(other) => {
              log::warn!("unable to parse anything in frame buffer - {other:?}");
              (Err(other), 0)
            }
          };

          if let Ok(header) = message_header {
            log::info!("parser started with {parser:?}");
            parser = header.chars().fold(parser, |p, c| p.take(c));
          }

          log::info!("parser concluded with {parser:?}");

          match &parser {
            // If we finished parsing the `message_header` as a bulk string that is yet to be read,
            // attempt to push into our actual image buffer that range of the slice starting from
            // where the header ended, to where the image is expected to end.
            MessageState::BulkString(BulkStringLocation::Reading(remainder, _, _), _) => {
              if header_size > 0 {
                let terminal = header_size + remainder;
                log::info!("image located @ {header_size} -> {terminal}");
                if frame_buffer.len() < terminal {
                  log::warn!("confused - header: '{header_size}' remainder: '{remainder}'");
                } else {
                  image_buffer.extend_from_slice(&frame_buffer[header_size..terminal]);
                }
                break 'response_read;
              }
            }
            MessageState::BulkString(BulkStringLocation::Sizing(0), Some((messages, arr_size)))
              if *arr_size == messages.len() =>
            {
              log::info!("found command = {messages:?}");
            }
            other => log::warn!("parser finished with unexpected result - {other:?}"),
          }
        }

        Err(error) if error.kind() == io::ErrorKind::TimedOut => {
          log::warn!("timeout while reading image pop - {error}");
          break 'response_read;
        }

        Err(error) => {
          log::warn!("unknown error while reading - {error}");
          return Err(error);
        }
      }
    }

    if !image_buffer.is_empty() {
      if let Err(error) = save_image(&args, image_buffer) {
        log::warn!("unable to save image - {error}");
      }
    }

    log::info!("writing message '{mock_device_id}' for keep-alive",);

    let response = kramer::execute(
      &mut connection,
      kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Push(
        (kramer::Side::Right, kramer::Insertion::Always),
        beetle::constants::REGISTRAR_INCOMING,
        kramer::Arity::One(&mock_device_id),
      )),
    )
    .await;

    if !matches!(response, Ok(kramer::Response::Item(kramer::ResponseValue::Integer(_)))) {
      log::warn!("received strange response from incoming push - {response:?}");
    }
  }
}

fn main() -> io::Result<()> {
  let load_env = std::fs::metadata(".env").map(|meta| meta.is_file()).unwrap_or(false);

  if load_env {
    let env_result = dotenv::dotenv();
    println!(".env loaded? {:?}", env_result.is_ok());
  }

  env_logger::init();
  let args = CommandLineArguments::parse();
  async_std::task::block_on(run(args))
}

#[cfg(test)]
mod tests {
  use super::{BulkStringLocation, MessageState};

  #[test]
  fn test_array_message_single() {
    let mut parser = MessageState::default();
    for token in "*1\r\n$2\r\nhi\r\n".chars() {
      parser = parser.take(token);
    }
    assert_eq!(
      parser,
      MessageState::BulkString(BulkStringLocation::Sizing(0), Some((vec!["hi".to_string()], 1)))
    );
  }

  #[test]
  fn test_array_message_many() {
    let mut parser = MessageState::default();
    let mut buffer = "*11\r\n".to_string();
    let mut expected = Vec::new();

    for _ in 0..11 {
      buffer.push_str("$2\r\nhi\r\n");
      expected.push("hi".to_string());
    }

    for token in buffer.chars() {
      parser = parser.take(token);
    }

    assert_eq!(
      parser,
      MessageState::BulkString(BulkStringLocation::Sizing(0), Some((expected, 11)))
    );
  }
}
