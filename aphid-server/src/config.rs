use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// A small production static-file server for sites built with aphid.
///
/// Every option can also be set through the environment variable shown;
/// a command-line flag wins over the environment.
#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct Config {
    /// Directory to serve.
    #[arg(long, env = "APHID_ROOT", default_value = "/site")]
    pub root: PathBuf,

    /// Port to listen on (all interfaces).
    #[arg(long, env = "PORT", default_value_t = 8080)]
    pub port: u16,

    /// Log output format on stdout.
    #[arg(long, env = "APHID_LOG_FORMAT", value_enum, default_value_t = LogFormat::Text)]
    pub log_format: LogFormat,

    /// Log filter, e.g. `info` or `info,aphid_server=debug`.
    #[arg(long, env = "APHID_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogFormat {
    /// Human-readable lines.
    Text,
    /// One JSON object per line.
    Json,
}
