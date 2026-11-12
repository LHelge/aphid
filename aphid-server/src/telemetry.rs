use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{Config, LogFormat, Result};

/// Process-wide tracing setup. Logs always go to stdout.
pub struct Telemetry;

impl Telemetry {
    /// Install the global tracing subscriber described by `config`.
    pub fn init(config: &Config) -> Result<Self> {
        let filter = EnvFilter::builder().parse(&config.log_level)?;
        let registry = tracing_subscriber::registry().with(filter);
        match config.log_format {
            LogFormat::Text => registry.with(fmt::layer()).init(),
            LogFormat::Json => registry
                .with(fmt::layer().json().flatten_event(true))
                .init(),
        }
        Ok(Self)
    }

    /// Flush and tear down telemetry before exit.
    pub fn shutdown(self) {}
}
