use std::io::{Error, ErrorKind, Result};

async fn run(addr: String) -> Result<()> {
  let contents = std::fs::read_to_string("env.toml")?;

  let config = toml::from_str::<beetle::api::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    Error::new(ErrorKind::Other, "bad-config")
  })?;

  let worker = beetle::api::Worker::from_config(config).await?;

  log::info!("starting worker @ {addr}");
  beetle::api::new(worker).listen(&addr).await
}

fn main() -> Result<()> {
  dotenv::dotenv().map_err(|error| Error::new(ErrorKind::Other, error))?;
  env_logger::init();

  let addr = std::env::var("BEETLE_WEB_ADDR")
    .ok()
    .or_else(|| std::env::args().nth(1))
    .unwrap_or_else(|| "0.0.0.0:8337".into());

  async_std::task::block_on(run(addr))
}
