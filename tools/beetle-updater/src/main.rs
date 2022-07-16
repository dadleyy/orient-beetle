use async_std::stream::StreamExt;
use clap::Parser;
use serde::Deserialize;
use std::io::Result;

#[derive(Deserialize, Debug)]
struct GithubUpdaterConfig {
  name: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "kind")]
enum UpdaterUnitConfig {
  #[serde(rename = "github-release")]
  GithubRelease(GithubUpdaterConfig),
}

#[derive(Deserialize, Debug)]
struct UpdaterConfig {
  units: Option<Vec<UpdaterUnitConfig>>,
}

#[derive(Parser, Deserialize)]
#[clap(author, version, about, long_about = None)]
struct UpdaterCommandLineOptions {
  #[clap(short, long, value_parser)]
  config: String,
}

async fn run(config: UpdaterConfig) -> Result<()> {
  let mut interval = async_std::stream::interval(std::time::Duration::from_secs(1));

  log::debug!("entering working loop for config: {config:?}");

  loop {
    interval.next().await;
    log::debug!("polling");
  }
}

fn main() -> Result<()> {
  let _ = dotenv::dotenv();
  env_logger::init();
  log::debug!("env loaded");

  let options = UpdaterCommandLineOptions::parse();
  let ex = async_executor::LocalExecutor::new();
  let config_content = std::fs::read(&options.config)?;
  let config = toml::from_slice::<UpdaterConfig>(&config_content)?;

  futures_lite::future::block_on(ex.run(run(config)))
}
