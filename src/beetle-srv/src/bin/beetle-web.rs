use clap::Parser;
use std::io;

/// The command line options themselves.
#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineOptions {
  /// The path to a local toml file that holds our configuration information.
  #[arg(short = 'c', long, default_value = "env.toml")]
  config: String,

  #[arg(short = 'a', long, default_value = "0.0.0.0:8337")]
  addr: String,
}

async fn run(addr: String, config: beetle::api::Configuration) -> io::Result<()> {
  let worker = beetle::api::Worker::from_config(config).await?;

  log::info!(
    "starting worker @ {addr} (version {})",
    option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev")
  );
  beetle::api::new(worker).listen(&addr).await
}

fn main() -> io::Result<()> {
  if let Err(error) = dotenv::dotenv() {
    eprintln!("[beetle-web warning] - unable to find '.env' - {error}");
  }
  env_logger::init();

  let options = CommandLineOptions::parse();
  let contents = std::fs::read_to_string(&options.config).map_err(|error| {
    io::Error::new(
      io::ErrorKind::Other,
      format!("unable to load config file '{}' - {error}", options.config),
    )
  })?;
  let config = toml::from_str::<beetle::api::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  async_std::task::block_on(run(options.addr, config))
}
