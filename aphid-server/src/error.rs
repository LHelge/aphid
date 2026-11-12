use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("site root {0} is not a directory")]
    RootNotDirectory(PathBuf),
    #[error("invalid log level: {0}")]
    LogLevel(#[from] tracing_subscriber::filter::ParseError),
    #[error("OpenTelemetry exporter: {0}")]
    Otlp(#[from] opentelemetry_otlp::ExporterBuildError),
    #[error("OTLP HTTP client: {0}")]
    HttpClient(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
