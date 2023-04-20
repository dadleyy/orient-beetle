use clap::Parser;
use serde::Deserialize;
use std::io;

/// Creates a render layout and rasterizes it. This is then sent to the device.
#[derive(Parser, Deserialize, PartialEq, Debug)]
pub struct SendImageCommand {
  /// The id of a device.
  #[arg(short = 'd', long)]
  id: String,
  /// The message to render.
  #[arg(short = 'm', long)]
  message: String,
  /// An optional path on the filesystem where the image can be written.
  #[arg(short = 'o', long)]
  local_path: Option<String>,
}

/// Builds and sends an image to a specific device.
pub async fn send_image(config: &super::CommandLineConfig, command: SendImageCommand) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let formatted_buffer = beetle::rendering::RenderLayout::Message(&command.message).rasterize((400, 300))?;

  if let Some(path) = &command.local_path {
    println!("writing image to {path}");
    let local_buffer = formatted_buffer.clone();
    let mut file = async_std::fs::File::create(&path).await.map_err(|error| {
      io::Error::new(
        error.kind(),
        format!("Unable to open file {path} for saving local copy of image - {error}"),
      )
    })?;
    async_std::io::WriteExt::write_all(&mut file, local_buffer.as_slice())
      .await
      .map_err(|error| {
        io::Error::new(
          error.kind(),
          format!("Unable to save file {path} for saving local copy of image - {error}"),
        )
      })?
  }

  let queue_id = beetle::redis::device_message_queue_id(command.id);

  let mut command = kramer::Command::Lists(kramer::ListCommand::Push(
    (kramer::Side::Left, kramer::Insertion::Always),
    queue_id.as_str(),
    kramer::Arity::One(formatted_buffer.as_slice().iter().enumerate()),
  ));

  println!("buffer size: {}", formatted_buffer.len());
  let result = command.execute(&mut stream).await?;

  match result {
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) => {
      println!("successfully wrote {amount} message(s) to '{queue_id}'")
    }
    other => {
      return Err(io::Error::new(
        io::ErrorKind::Other,
        format!("Strange response from device '{queue_id}' queue - {other:?}"),
      ))
    }
  }
  Ok(())
}

/// Prints the amount of items pending in a queue for a given device id.
pub async fn print_queue_size(
  config: &super::CommandLineConfig,
  command: super::SingleDeviceCommand,
) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let queue_id = beetle::redis::device_message_queue_id(command.id);
  let command = kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Len(queue_id.as_str()));
  let result = kramer::execute(&mut stream, &command).await?;

  match result {
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) => {
      println!("device '{queue_id}' has {amount} queued items")
    }
    unknown => {
      return Err(io::Error::new(
        io::ErrorKind::Other,
        format!("Strange response from device '{queue_id}' queue - {unknown:?}"),
      ))
    }
  }
  Ok(())
}
