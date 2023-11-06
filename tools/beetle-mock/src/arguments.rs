use clap::Parser;

#[derive(Parser)]
#[command(author, version = option_env!("BEETLE_VERSION").unwrap_or_else(|| "dev"), about, long_about = None)]
pub struct CommandLineArguments {
  #[clap(short, long, default_value = "env.toml")]
  pub config: String,

  #[clap(short, long, default_value = ".storage")]
  pub storage: String,
}
