#![warn(clippy::missing_docs_in_private_items)]

//! This command line tool is meant to be a quick-and-dirty quality of life improvement over
//! working through the webserver + ui.

use clap::Parser;
use serde::Deserialize;
use std::io;

/// Code organization; this submodule breaks out commands.
mod cli;

/// The various sub-commands of our cli.
#[derive(PartialEq, clap::Subcommand, Deserialize, Default)]
enum CommandLineCommand {
  /// Prints connected devices.
  #[default]
  PrintConnected,

  /// This command will blow away _all_ current acl entries. At that moment, devices will need to
  /// re-authenticate from a fresh set of available ids.
  InvalidateAcls,

  /// Removes devices that have not been heard from within the amount of time that we consider
  /// active.
  CleanDisconnects,

  /// Removes all collections.
  DropCollections,

  /// Creates the ACL entries that will be used by devices for requesting their unique identifiers.
  Provision(cli::ProvisionCommand),

  /// Creates request for an image render and queues it.
  SendImage(cli::SendImageCommand),

  /// Creates request for an layout render and queues it.
  SendLayout(cli::SendLayoutCommand),

  /// Creates request for a qr code render and queues it.
  SendScannable(cli::SendScannableCommand),

  /// Resets the device registration state so the renderer will send a new registration qr code.
  ResetRegistration(cli::SingleDeviceCommand),

  /// Turns the lights on.
  Darken(cli::SingleDeviceCommand),

  /// Turns the lights off.
  Lighten(cli::SingleDeviceCommand),

  /// Prints the length of a device message queue.
  PrintItems(cli::SingleDeviceCommand),
}

/// The command line options themselves.
#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineOptions {
  /// The path to a local toml file that holds our configuration information.
  #[arg(short = 'c', long, default_value = "env.toml")]
  config: String,

  /// The subcommand.
  #[command(subcommand)]
  command: CommandLineCommand,
}

/// The main async cli runtime.
async fn run(config: cli::CommandLineConfig, command: CommandLineCommand) -> io::Result<()> {
  println!("== cli context");
  println!("  redis:   {}:{}", config.redis.host, config.redis.port);
  println!("  mongofb: {}", config.mongo.url);
  println!("==");
  match command {
    CommandLineCommand::DropCollections => {
      let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
      mongo
        .database(&config.mongo.database)
        .collection::<beetle::types::User>(&config.mongo.collections.users)
        .drop(None)
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
      mongo
        .database(&config.mongo.database)
        .collection::<beetle::types::DeviceAuthorityRecord>(&config.mongo.collections.device_authorities)
        .drop(None)
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
      mongo
        .database(&config.mongo.database)
        .collection::<beetle::types::DeviceDiagnostic>(&config.mongo.collections.device_diagnostics)
        .drop(None)
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
      Ok(())
    }
    CommandLineCommand::InvalidateAcls => cli::invalidate_acls(&config).await,
    CommandLineCommand::CleanDisconnects => cli::clean_disconnects(&config).await,
    CommandLineCommand::Provision(command) => cli::provision(&config, command).await,
    CommandLineCommand::PrintConnected => cli::print_connected(&config).await,
    inner @ CommandLineCommand::Darken(_) | inner @ CommandLineCommand::Lighten(_) => {
      log::info!("toggling light state");
      let mut stream = beetle::redis::connect(&config.redis).await?;
      let (id, inner) = match &inner {
        CommandLineCommand::Darken(inner) => (&inner.id, beetle::rendering::RenderVariant::off()),
        CommandLineCommand::Lighten(inner) => (&inner.id, beetle::rendering::RenderVariant::on()),
        _ => unreachable!(),
      };
      let mut queue = beetle::rendering::queue::Queue::new(&mut stream);
      let (request_id, pending) = queue
        .queue::<&str, &str>(id, &beetle::rendering::queue::QueuedRenderAuthority::CommandLine, inner)
        .await?;
      log::info!("id '{request_id}' | pending {pending}");

      Ok(())
    }
    CommandLineCommand::PrintItems(cmd) => cli::print_queue_size(&config, cmd).await,
    CommandLineCommand::SendImage(cmd) => cli::send_image(&config, cmd).await,
    CommandLineCommand::SendLayout(cmd) => cli::send_layout(&config, cmd).await,
    CommandLineCommand::SendScannable(cmd) => cli::send_scannable(&config, cmd).await,
    CommandLineCommand::ResetRegistration(cmd) => {
      log::info!("resetting device '{}' to force qr code", cmd.id);
      let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
      let collection = mongo
        .database(&config.mongo.database)
        .collection::<beetle::types::DeviceDiagnostic>(&config.mongo.collections.device_diagnostics);
      let updated_reg = beetle::types::DeviceDiagnosticRegistration::Initial;
      let serialized_registration = bson::to_bson(&updated_reg).map_err(|error| {
        log::warn!("unable to serialize registration_state: {error}");
        io::Error::new(io::ErrorKind::Other, format!("{error}"))
      })?;
      if let Err(error) = collection
        .find_one_and_update(
          bson::doc! { "id": &cmd.id },
          bson::doc! { "$set": { "registration_state": serialized_registration } },
          mongodb::options::FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        )
        .await
      {
        log::warn!("unable to update device registration state - {error}");
      }

      Ok(())
    }
  }
}

/// The entrypoint.
fn main() -> io::Result<()> {
  dotenv::dotenv().ok();
  env_logger::init();

  log::info!("environment + logger ready.");

  let options = CommandLineOptions::parse();
  let contents = std::fs::read_to_string(&options.config)?;
  let config = toml::from_str::<cli::CommandLineConfig>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  async_std::task::block_on(run(config, options.command))
}
