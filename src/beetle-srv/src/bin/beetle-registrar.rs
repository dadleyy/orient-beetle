use async_std::prelude::*;
use clap::Parser;
use std::io::{Error, ErrorKind, Result};

#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineArguments {
  #[clap(short, long, default_value = "env.toml")]
  config: String,
}

async fn run(config: beetle::registrar::Configuration) -> Result<()> {
  log::info!(
    "starting registrar from (version {})",
    option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev")
  );

  let mut failures = Vec::with_capacity(3);
  let interval_ms = config.registrar.interval_delay_ms.as_ref().copied();
  let mut interval = interval_ms
    .map(std::time::Duration::from_millis)
    .map(async_std::stream::interval);

  let mut last_debug = std::time::Instant::now();
  let mut frames = 0u8;
  let mut worker = config.worker().await?;

  while failures.len() < 10 {
    log::trace!("attempting worker frame");

    let now = std::time::Instant::now();
    if now.duration_since(last_debug).as_secs() > 4 || frames == u8::MAX {
      last_debug = now;
      log::info!("registar still working ({frames} frames since last interval)...");
      frames = 0;
    }

    frames += 1;

    match worker.work().await {
      Err(error) => failures.push(format!("{error}")),
      Ok(()) => {
        if !failures.is_empty() {
          log::info!("recovered from failures: {}", failures.drain(0..).collect::<String>());
        }
      }
    }

    if let Some(interval) = interval.as_mut() {
      log::trace!("explicit registrar execution delay - {:?}", interval_ms);
      interval.next().await;
    }
  }

  log::warn!("registrar exiting with failures - {failures:?}");

  Ok(())
}

fn main() -> Result<()> {
  let load_env = std::fs::metadata(".env").map(|meta| meta.is_file()).unwrap_or(false);

  if load_env {
    let env_result = dotenv::dotenv();
    println!(".env loaded? {:?}", env_result);
  }

  env_logger::init();
  log::info!("environment + logger ready.");

  let args = CommandLineArguments::parse();
  let contents = std::fs::read_to_string(args.config)?;

  let config = toml::from_str::<beetle::registrar::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    Error::new(ErrorKind::Other, "bad-config")
  })?;

  async_std::task::block_on(run(config))
}
