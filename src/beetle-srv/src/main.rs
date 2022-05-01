use std::io::{Error, ErrorKind, Result};

fn main() -> Result<()> {
  dotenv::dotenv().map_err(|error| Error::new(ErrorKind::Other, error))?;
  env_logger::init();

  log::info!("environment + logger ready.");

  Ok(())
}
