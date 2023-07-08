use clap::Parser;
use serde::Deserialize;
use std::io;

/// Creates a scannable render layout and queues it.
#[derive(Parser, Deserialize, PartialEq, Debug)]
pub struct SendScannableCommand {
  /// The id of a device.
  #[arg(short = 'd', long)]
  id: String,
  /// The message to render.
  #[arg(short = 'c', long)]
  content: String,
  /// An optional path on the filesystem where the image can be written.
  #[arg(short = 'o', long)]
  local_path: Option<String>,
}

/// Attempts to parse a json file as some render layout and send it.
#[derive(Parser, Deserialize, PartialEq, Debug)]
pub struct SendLayoutCommand {
  /// The id of a device. If this is omitted, it is assumed we are creating an image for the local
  /// filesystem.
  #[arg(short = 'd', long)]
  id: Option<String>,
  /// The message to render.
  #[arg(short = 'i', long)]
  layout_file: String,
  /// An optional path on the filesystem where the image can be written. This _must_ be present if
  /// a device id is omitted.
  #[arg(short = 'o', long)]
  local_path: Option<String>,
}

/// Creates a render layout and rasterizes it. This is then sent to the device.
#[derive(Parser, Deserialize, PartialEq, Debug)]
pub struct SendImageCommand {
  /// The id of a device. If this is omitted, it is assumed we are creating an image for the local
  /// filesystem.
  #[arg(short = 'd', long)]
  id: Option<String>,
  /// The message to render.
  #[arg(short = 'm', long)]
  message: String,
  /// An optional path on the filesystem where the image can be written. This _must_ be present if
  /// a device id is omitted.
  #[arg(short = 'o', long)]
  local_path: Option<String>,
}

/// Attempts to parse and send a layout to a device.
pub async fn send_layout(config: &super::CommandLineConfig, command: SendLayoutCommand) -> io::Result<()> {
  if command.id.is_none() && command.local_path.is_none() {
    return Err(io::Error::new(
      io::ErrorKind::Other,
      "please provide device id or output path",
    ));
  }

  let file_contents = async_std::fs::read_to_string(&command.layout_file).await?;
  let parsed_layout =
    serde_json::from_str::<beetle::rendering::RenderLayout<String>>(&file_contents).map_err(|error| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("invalid layout file '{}': {error}", command.layout_file),
      )
    })?;

  log::info!("parsed layout - {parsed_layout:?}");

  if let Some(path) = &command.local_path {
    println!("writing image to {path}");
    let local_buffer = parsed_layout.clone().rasterize((400, 300))?;
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

  if let Some(device_id) = &command.id {
    let mut stream = beetle::redis::connect(&config.redis).await?;
    let mut queue = beetle::rendering::Queue::new(&mut stream);
    let (request_id, pending) = queue
      .queue(
        device_id,
        &beetle::rendering::QueuedRenderAuthority::CommandLine,
        beetle::rendering::RenderVariant::layout(parsed_layout),
      )
      .await?;

    println!(
      "message queued successfully. id '{request_id}' ({}/{} in queue)",
      pending + 1,
      pending + 1
    );
  }

  Ok(())
}

/// Builds and sends an image to a specific device.
pub async fn send_scannable(config: &super::CommandLineConfig, command: SendScannableCommand) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let layout = beetle::rendering::RenderLayout::Scannable(beetle::rendering::components::Scannable {
    contents: &command.content,
  });

  if let Some(path) = &command.local_path {
    println!("writing image to {path}");
    let local_buffer = layout.clone().rasterize((400, 300))?;
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

  let request = beetle::rendering::RenderVariant::scannable(&command.content);
  let mut queue = beetle::rendering::Queue::new(&mut stream);
  let (request_id, pending) = queue
    .queue(
      &command.id,
      &beetle::rendering::QueuedRenderAuthority::CommandLine,
      request,
    )
    .await?;

  println!(
    "message queued successfully. id '{request_id}' ({}/{} in queue)",
    pending + 1,
    pending + 1
  );

  Ok(())
}

/// Builds and sends an image to a specific device.
pub async fn send_image(config: &super::CommandLineConfig, command: SendImageCommand) -> io::Result<()> {
  if command.id.is_none() && command.local_path.is_none() {
    return Err(io::Error::new(
      io::ErrorKind::Other,
      "please provide device id or output path",
    ));
  }

  #[allow(deprecated)]
  let formatted_buffer =
    beetle::rendering::RenderLayout::StylizedMessage(beetle::rendering::components::StylizedMessage {
      message: &command.message,
      border: None,
      margin: None,
      padding: None,
      font: None,
      size: None,
    })
    .rasterize((400, 300))?;

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

  if let Some(device_id) = &command.id {
    let mut stream = beetle::redis::connect(&config.redis).await?;
    let mut queue = beetle::rendering::Queue::new(&mut stream);
    let (request_id, pending) = queue
      .queue(
        device_id,
        &beetle::rendering::QueuedRenderAuthority::CommandLine,
        beetle::rendering::RenderVariant::message(&command.message),
      )
      .await?;

    println!(
      "message queued successfully. id '{request_id}' ({}/{} in queue)",
      pending + 1,
      pending + 1
    );
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
