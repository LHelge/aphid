use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "aphid", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = "aphid.toml", global = true)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Build {
        /// Output directory for the rendered site.
        #[arg(short, long, default_value = "dist")]
        output: PathBuf,
    },
    Serve {
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },
    /// Create a new aphid site in a new directory.
    New {
        /// Name of the directory to create.
        name: String,
    },
    /// Initialize an aphid site in the current directory.
    Init {
        /// Directory to initialize (defaults to current directory).
        path: Option<PathBuf>,
    },
    /// Manage blog posts.
    Blog {
        #[command(subcommand)]
        action: BlogAction,
    },
    /// Manage wiki pages.
    Wiki {
        #[command(subcommand)]
        action: WikiAction,
    },
    /// Manage standalone pages.
    Page {
        #[command(subcommand)]
        action: PageAction,
    },
}

#[derive(Subcommand)]
pub enum BlogAction {
    /// Create a new blog post.
    New {
        /// Title of the blog post.
        title: String,
    },
}

#[derive(Subcommand)]
pub enum WikiAction {
    /// Create a new wiki page.
    New {
        /// Title of the wiki page.
        title: String,
    },
}

#[derive(Subcommand)]
pub enum PageAction {
    /// Create a new standalone page.
    New {
        /// Title of the page.
        title: String,
    },
}
