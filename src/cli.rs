use aphid::agent::AgentTool;
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
        /// Also write AI-agent instruction files. Omit the value for a generic AGENTS.md.
        #[arg(long, value_name = "TOOL", num_args = 0..=1, default_missing_value = "codex")]
        agent: Option<AgentTool>,
    },
    /// Initialize an aphid site in the current directory.
    Init {
        /// Directory to initialize (defaults to current directory).
        path: Option<PathBuf>,
        /// Also write AI-agent instruction files. Omit the value for a generic AGENTS.md.
        #[arg(long, value_name = "TOOL", num_args = 0..=1, default_missing_value = "codex")]
        agent: Option<AgentTool>,
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
    /// Write AI-agent instruction files for this site.
    ///
    /// Omit the tool argument for a generic AGENTS.md — recognised by Codex, Aider,
    /// Goose, and current Cursor.
    Agent {
        /// Target agent.
        #[arg(default_value = "codex")]
        tool: AgentTool,
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
