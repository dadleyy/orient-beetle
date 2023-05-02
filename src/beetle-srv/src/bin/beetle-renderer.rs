use clap::Parser;
use std::io;

#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
struct CommandLineArguments {
  #[clap(short, long)]
  config: String,
}

async fn run(args: CommandLineArguments) -> io::Result<()> {
  log::info!("attempting to load '{}'", args.config);

  let contents = async_std::fs::read_to_string(&args.config).await?;
  let config = toml::from_str::<beetle::registrar::Configuration>(&contents).map_err(|error| {
    log::warn!("invalid toml config file - {error}");
    io::Error::new(io::ErrorKind::Other, "bad-config")
  })?;

  beetle::rendering::renderer::run(config).await
}

fn main() -> io::Result<()> {
  let load_env = std::fs::metadata(".env").map(|meta| meta.is_file()).unwrap_or(false);

  if load_env {
    let env_result = dotenv::dotenv();
    println!(".env loaded? {:?}", env_result.is_ok());
  }

  env_logger::init();
  log::info!("environment + logger ready.");
  let args = CommandLineArguments::parse();

  async_std::task::block_on(run(args))
}
