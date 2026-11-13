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

    /// Log output format on stdout. `none` turns stdout logging off, e.g.
    /// when logs only go to OpenTelemetry.
    #[arg(long, env = "APHID_LOG_FORMAT", value_enum, default_value_t = LogFormat::Text)]
    pub log_format: LogFormat,

    /// Log filter, e.g. `info` or `info,aphid_server=debug`.
    #[arg(long, env = "APHID_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// OTLP/HTTP base URL (e.g. `http://collector:4318`). Enables OpenTelemetry
    /// trace and log export; without it nothing is exported.
    #[arg(long, env = "OTEL_EXPORTER_OTLP_ENDPOINT")]
    pub otlp_endpoint: Option<String>,
}

impl Config {
    /// The configured OTLP endpoint, treating an empty value as unset.
    pub fn otlp_endpoint(&self) -> Option<&str> {
        self.otlp_endpoint
            .as_deref()
            .filter(|e| !e.trim().is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogFormat {
    /// Human-readable lines.
    Text,
    /// One JSON object per line.
    Json,
    /// Nothing on stdout.
    None,
}
