#![deny(unsafe_code)]

//! This executable is meant to provide support for local development; this application mirrors the
//! platformio firmware running on the embedded device - images will be pulled + downloaded to the
//! local filesystem.

use clap::Parser;
use iced::Application;
use std::io;

mod arguments;
use arguments::CommandLineArguments;

mod id;
use id::get_device_id;

mod redis_reader;
use redis_reader::{MessageState, RedisResponse};

fn save_image(args: &CommandLineArguments, image_buffer: &Vec<u8>) -> io::Result<()> {
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

async fn run_background(args: CommandLineArguments, s: async_std::channel::Sender<Vec<u8>>) -> io::Result<()> {
  let contents = async_std::fs::read_to_string(&args.config).await?;
  let config = toml::from_str::<beetle::registrar::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  log::info!("mock starting with config - '{config:?}'");

  let mut connection = beetle::redis::connect(&config.redis).await.map_err(|error| {
    log::error!("unable to connect to redis broker - {error:?}");
    io::Error::new(
      io::ErrorKind::Other,
      format!("failed redis connection '{:?}' - {error}", config.redis),
    )
  })?;

  let mock_device_id = get_device_id(&args, &config, &mut connection).await.map_err(|error| {
    log::error!("unable to get device id - {error:?}");
    error
  })?;

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

    let mut image_buffer: Vec<u8> = Vec::with_capacity(1024 * 104);
    let mut parser = MessageState::default();

    log::info!("pop written, waiting for response");
    let mut read_count = 0;

    // TODO: this does not seem very structurally sound; the goal is to read from the redis
    // connection, attempting to parse our pop messages as a payload of image data.
    'response_read: loop {
      read_count += 1;
      let mut frame_buffer = [0u8; 1024 * 80];

      let response = match async_std::io::timeout(
        std::time::Duration::from_secs(6),
        async_std::io::ReadExt::read(&mut connection, &mut frame_buffer),
      )
      .await
      {
        Ok(read_amount) => {
          log::info!("read {read_amount} bytes on read#{read_count}");

          parser = frame_buffer[0..read_amount]
            .iter()
            .fold(parser, |current_parser, token| current_parser.take(*token));

          if let MessageState::Complete(ref terminal) = parser {
            terminal
          } else {
            log::info!("message reading concluded incomplete parser, preparing for read {read_count}");
            continue;
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
      };

      match response {
        RedisResponse::Array(ref items) if items.len() == 2 => {
          let queue_name = String::from_utf8(items.get(0).unwrap().clone());
          log::info!("has image from {queue_name:?}");
          let buffer = items.get(1).unwrap();
          image_buffer.extend_from_slice(buffer.as_slice());
        }
        RedisResponse::Array(_) => {
          log::trace!("empty array response");
        }
        other => {
          log::warn!("unhandled redis response - {other:?}");
        }
      }

      break;
    }

    if !image_buffer.is_empty() {
      if let Err(error) = save_image(&args, &image_buffer) {
        log::warn!("unable to save image - {error}");
      }

      s.send(image_buffer)
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
    }

    log::info!("writing message '{mock_device_id}' for keep-alive");

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

struct BeetleMock {
  receiver: async_std::sync::Arc<async_std::sync::Mutex<async_std::channel::Receiver<Vec<u8>>>>,
  latest_image: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
enum BeetleMessage {
  Clear,
  SetImage(Vec<u8>),
}

struct BeetleInit {
  receiver: async_std::channel::Receiver<Vec<u8>>,
}

impl iced::Application for BeetleMock {
  type Message = BeetleMessage;
  type Executor = iced::executor::Default;
  type Theme = iced::Theme;
  type Flags = BeetleInit;

  fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
    let receiver = async_std::sync::Arc::new(async_std::sync::Mutex::new(flags.receiver));
    (
      Self {
        receiver,
        latest_image: None,
      },
      iced::Command::none(),
    )
  }

  fn subscription(&self) -> iced::Subscription<Self::Message> {
    log::info!("creating subscription");
    let receiver = self.receiver.clone();
    struct Worker;
    iced::subscription::unfold(std::any::TypeId::of::<Worker>(), receiver, |r| async move {
      async_std::task::sleep(std::time::Duration::from_millis(1000)).await;
      log::info!("attempting to lock + receive");
      let next = match r.lock().await.recv().await {
        Ok(buffer) => BeetleMessage::SetImage(buffer),
        Err(error) => {
          log::error!("unable to pull next message - {error}");
          BeetleMessage::Clear
        }
      };
      (next, r)
    })
  }

  fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
    #[allow(clippy::single_match)]
    match message {
      Self::Message::SetImage(buffer) => self.latest_image = Some(buffer),
      _ => (),
    }
    iced::Command::none()
  }

  fn view(&self) -> iced::Element<Self::Message> {
    if let Some(buffer) = self.latest_image.as_ref() {
      let loaded_image = image::guess_format(buffer.as_slice());
      log::info!("has buffer from latest - {}: {loaded_image:?}", buffer.len());

      let handle = iced::widget::image::Handle::from_memory(buffer.clone());
      let img = iced::widget::image::viewer(handle);
      return iced::widget::column![img].into();
    }

    iced::widget::column![].into()
  }

  fn title(&self) -> String {
    "hello".to_string()
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
  let (s, r) = async_std::channel::unbounded();

  async_std::task::spawn(run_background(args, s));

  let mut settings = iced::Settings::with_flags(BeetleInit { receiver: r });
  settings.window.max_size = Some((400, 300));
  settings.window.resizable = false;
  BeetleMock::run(settings).map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
}
