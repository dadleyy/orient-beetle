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

async fn run(args: CommandLineArguments) -> io::Result<()> {
  let contents = async_std::fs::read_to_string(&args.config).await?;
  let config = toml::from_str::<beetle::registrar::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  let (id_user, id_password) = config
    .registrar
    .id_consumer_username
    .zip(config.registrar.id_consumer_password)
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        "Configuration is missing registrar burn-in credentials",
      )
    })?;

  let mut id_storage_path = std::path::PathBuf::from(&args.storage);
  id_storage_path.push(".device_id");

  let mut connection = beetle::redis::connect(&config.redis).await?;

  let mock_device_id = match async_std::fs::metadata(&id_storage_path).await {
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

      mock_device_id
    }

    Ok(meta) if meta.is_file() => {
      log::info!("found existing device id at '{:?}'", id_storage_path);
      let loaded_id = async_std::fs::read_to_string(&id_storage_path).await?;
      log::info!("loaded device id - '{loaded_id}'");

      loaded_id
    }
    other @ Ok(_) | other @ Err(_) => {
      return Err(io::Error::new(
        io::ErrorKind::Other,
        format!("unable to handle device id storage lookup - {other:?}"),
      ))
    }
  };

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

    let mut frame_size = 0usize;
    let mut image_buffer: Vec<u8> = Vec::with_capacity(1024 * 10);
    let mut parser = MessageState::default();

    log::info!("pop written, waiting for response");

    'response_read: loop {
      let mut frame_buffer = [0u8; 1024 * 8];

      match async_std::io::timeout(
        std::time::Duration::from_secs(6),
        async_std::io::ReadExt::read(&mut connection, &mut frame_buffer),
      )
      .await
      {
        Ok(amount) => {
          frame_size += amount;

          if amount == 5 && matches!(std::str::from_utf8(&frame_buffer[0..amount]), Ok("*-1\r\n")) {
            log::info!("empty read from redis, moving on immediately");
            frame_size = 0;
            break 'response_read;
          }

          if amount > 3 {
            let first_couple = match std::str::from_utf8(&frame_buffer) {
              Ok(inner) => Ok(inner),
              Err(error) if error.valid_up_to() > 0 => std::str::from_utf8(&frame_buffer[0..error.valid_up_to()]),
              Err(other) => {
                log::warn!("unable to parse anything in frame buffer - {other:?}");
                Err(other)
              }
            };

            if let Ok(header) = first_couple {
              for letter in header.chars() {
                parser = parser.take(letter);
              }
            }

            log::info!("first couple - {first_couple:?} ({parser:?})");
          }

          image_buffer.extend_from_slice(&frame_buffer[0..amount]);
          log::info!("read {amount} byte(s)");
        }
        Err(error) if error.kind() == io::ErrorKind::TimedOut => {
          log::warn!("timeout while reading image pop - {error} (after {frame_size} byte(s))");
          break 'response_read;
        }
        Err(error) => {
          log::warn!("unknown error while reading - {error}");
          return Err(error);
        }
      }
    }

    if frame_size > 0 {
      log::info!("had some data in our frame, attempting to parse as png");
      match image::codecs::png::PngDecoder::new(std::io::Cursor::new(image_buffer)) {
        Ok(decoder) => {
          let dims = image::ImageDecoder::dimensions(&decoder);
          log::info!("found image - {:?}", dims);
        }
        Err(error) => {
          log::warn!("unable to decode as image - {error}");
        }
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
