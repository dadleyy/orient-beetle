use async_std::prelude::*;
use std::io::{Error, ErrorKind, Result};

async fn run(config: beetle::registrar::Configuration) -> Result<()> {
  let mut worker = config.worker().await?;
  let mut failures = Vec::with_capacity(3);
  let mut interval = async_std::stream::interval(std::time::Duration::from_millis(1000));

  while failures.len() < 10 {
    interval.next().await;
    log::trace!("attempting worker frame");

    match worker.work().await {
      Err(error) => failures.push(format!("{error}")),
      Ok(()) => {
        if failures.len() != 0 {
          log::info!("recovered from failures: {}", failures.drain(0..).collect::<String>());
        }
      }
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

  let contents = std::fs::read_to_string("env.toml")?;

  let config = toml::from_str::<beetle::registrar::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    Error::new(ErrorKind::Other, "bad-config")
  })?;

  async_std::task::block_on(run(config))
}
