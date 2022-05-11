use std::io::{Error, ErrorKind, Result};

async fn run(addr: String) -> Result<()> {
  let contents = std::fs::read_to_string("env.toml")?;

  let config = toml::from_str::<beetle::api::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    Error::new(ErrorKind::Other, "bad-config")
  })?;

  let worker = config.worker().await?;

  beetle::api::new(worker).listen(&addr).await
}

fn main() -> Result<()> {
  dotenv::dotenv().map_err(|error| Error::new(ErrorKind::Other, error))?;
  env_logger::init();

  let addr = std::env::var("BEETLE_WEB_ADDR")
    .ok()
    .or(std::env::args().skip(1).next())
    .unwrap_or("0.0.0.0:8337".into());

  async_std::task::block_on(run(addr))
}
