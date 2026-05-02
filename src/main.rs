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
        Command::New { name } => aphid::scaffold_new(&name),
        Command::Init { path } => {
            aphid::scaffold_init(&path.unwrap_or_else(|| std::path::PathBuf::from(".")))
        }
        Command::Blog { action } => match action {
            cli::BlogAction::New { title } => aphid::new_blog_post(&cli.config, &title),
        },
        Command::Wiki { action } => match action {
            cli::WikiAction::New { title } => aphid::new_wiki_page(&cli.config, &title),
        },
        Command::Page { action } => match action {
            cli::PageAction::New { title } => aphid::new_page(&cli.config, &title),
        },
    }
}
