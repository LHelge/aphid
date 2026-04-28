mod cli;

use aphid::Error;
use clap::Parser;
use cli::{Cli, Command};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve { port: 3000 }) {
        Command::Build { output } => aphid::build(&cli.config, &output).await,
        Command::Serve { port } => aphid::serve(&cli.config, port).await,
    }
}
