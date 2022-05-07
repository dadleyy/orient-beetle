use std::io::{Error, ErrorKind, Result};

#[derive(Default, Clone)]
struct CommandLineConfig {
  addr: String,
  redis: (String, String, String),
}

/// Having fun with traits, not necessarily how this will be in the long-term.
impl beetle::api::Connector for CommandLineConfig {
  type Value = String;

  fn redis<'a>(&'a self) -> (&'a String, &'a String, &'a String) {
    (&self.redis.0, &self.redis.1, &self.redis.2)
  }
}

async fn run(config: CommandLineConfig) -> Result<()> {
  let addr = format!("{}", config.addr);
  beetle::api::new(config).listen(&addr).await
}

fn main() -> Result<()> {
  dotenv::dotenv().map_err(|error| Error::new(ErrorKind::Other, error))?;
  env_logger::init();

  let mut config = CommandLineConfig::default();
  log::info!("environment + logger ready.");

  let redis = std::env::var("REDIS_HOST")
    .ok()
    .zip(std::env::var("REDIS_PORT").ok())
    .zip(std::env::var("REDIS_AUTH").ok())
    .map(|((h, p), a)| (h, p, a));

  if let Some(redis) = redis {
    config.redis = redis;
  }

  if let Some(addr) = std::env::var("BEETLE_WEB_ADDR").ok() {
    config.addr = addr;
  }

  async_std::task::block_on(run(config))
}
