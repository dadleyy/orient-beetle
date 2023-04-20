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

  /// Creates the ACL entries that will be used by devices for requesting their unique identifiers.
  Provision(cli::ProvisionCommand),

  /// Creates an image and sends it along to the device.
  SendImage(cli::SendImageCommand),

  /// Prints the length of a device message queue.
  PrintItems(cli::SingleDeviceCommand),
}

/// The command line options themselves.
#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineOptions {
  /// The path to a local toml file that holds our configuration information.
  #[arg(short = 'c', long)]
  config: String,

  /// The subcommand.
  #[command(subcommand)]
  command: CommandLineCommand,
}

/// The main async cli runtime.
async fn run(config: cli::CommandLineConfig, command: CommandLineCommand) -> io::Result<()> {
  match command {
    CommandLineCommand::InvalidateAcls => cli::invalidate_acls(&config).await,
    CommandLineCommand::CleanDisconnects => cli::clean_disconnects(&config).await,
    CommandLineCommand::Provision(command) => cli::provision(&config, command).await,
    CommandLineCommand::PrintConnected => cli::print_connected(&config).await,
    CommandLineCommand::PrintItems(cmd) => cli::print_queue_size(&config, cmd).await,
    CommandLineCommand::SendImage(cmd) => cli::send_image(&config, cmd).await,
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
