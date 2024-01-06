use clap::Parser;
use serde::Deserialize;

/// Some commands operate on a single device; this structure is shared across those.
#[derive(Parser, Deserialize, PartialEq, Debug)]
pub struct SingleDeviceCommand {
  /// The id of a device.
  #[arg(short = 'd', long)]
  pub id: String,
}

/// Similar to the struct provided by the library code in this crate, this structure is used by the
/// cli tool's internal configuration `toml` schema.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RegistrarConfiguration {
  /// The username portion of the `AUTH` acl entries that is burned-in on devices.
  pub id_consumer_username: Option<String>,
  /// The password portion of the `AUTH` acl entries that is burned-in on devices.
  pub id_consumer_password: Option<String>,
  /// The list of acl entries that should _not_ be invalidated when cleaning up.
  pub acl_user_allowlist: Option<Vec<String>>,
  /// The secret used to encrypt vendor api access tokens.
  pub vendor_api_secret: String,
}

/// The CLI tool's internal configuration schema; this should basically mirror the same structure
/// defined in the library, but is likely to be a subset.
#[derive(Deserialize, Debug)]
pub struct CommandLineConfig {
  /// The redis configuration.
  pub redis: beetle::config::RedisConfiguration,
  /// The mongodb configuration.
  pub mongo: beetle::config::MongoConfiguration,
  /// Configuration specific for the registrar.
  pub registrar: RegistrarConfiguration,
}

/// Commands associated with device connectivity/activity.
mod disconnects;
pub use disconnects::{clean_disconnects, print_connected};

/// A list of migrations
pub mod migrate;

/// Commands associated with device permissions + authentication.
mod acls;
pub use acls::{invalidate_acls, print_acls, provision, ProvisionCommand};

/// Commands associated with device messaging.
mod messages;
pub use messages::{
  print_queue_size, send_image, send_layout, send_scannable, SendImageCommand, SendLayoutCommand, SendScannableCommand,
};
