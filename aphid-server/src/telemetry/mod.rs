//! Process-wide tracing setup.
//!
//! Stdout logging is always on. OpenTelemetry export (traces and logs over
//! OTLP/HTTP) is added on top only when an endpoint is configured.

mod otlp;

use std::time::Duration;

use tracing::Subscriber;
use tracing_subscriber::{
    EnvFilter, Layer, fmt, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
};

pub(crate) use otlp::OtlpPipeline;

use crate::{Config, LogFormat, Result, access_log::HTTP_SPAN_TARGET};

/// How long graceful shutdown waits for buffered telemetry to flush.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(3);

/// Targets of the OpenTelemetry SDK's own diagnostics. Silenced unless the
/// log filter mentions them, so a dead collector cannot flood stdout;
/// [`otlp`] reports export failures once instead.
const OTEL_INTERNAL_TARGETS: &[&str] = &[
    "opentelemetry",
    "opentelemetry_sdk",
    "opentelemetry_otlp",
    "opentelemetry_http",
    "opentelemetry_appender_tracing",
    "tracing_opentelemetry",
];

pub struct Telemetry {
    otlp: Option<OtlpPipeline>,
}

impl Telemetry {
    /// Install the global tracing subscriber described by `config`.
    ///
    /// Call this outside the tokio runtime: the OTLP exporters use a blocking
    /// HTTP client on their own threads.
    pub fn init(config: &Config) -> Result<Self> {
        let otlp = config
            .otlp_endpoint()
            .map(OtlpPipeline::from_endpoint)
            .transpose()?;
        let otel_layers = otlp
            .as_ref()
            .map(|pipeline| pipeline.layers(&config.log_level))
            .transpose()?;

        tracing_subscriber::registry()
            .with(Self::stdout_layer(config)?)
            .with(otel_layers)
            .init();

        if let Some(endpoint) = config.otlp_endpoint() {
            tracing::info!(endpoint, "OpenTelemetry export enabled (OTLP/HTTP)");
        }
        Ok(Self { otlp })
    }

    /// Flush buffered telemetry (bounded by a short timeout) and tear down.
    pub fn shutdown(self) {
        if let Some(pipeline) = self.otlp {
            pipeline.shutdown(FLUSH_TIMEOUT);
        }
    }

    fn stdout_layer<S>(config: &Config) -> Result<Box<dyn Layer<S> + Send + Sync>>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let filter = Self::log_filter(&config.log_level)?;
        Ok(match config.log_format {
            LogFormat::Text => fmt::layer().with_filter(filter).boxed(),
            LogFormat::Json => fmt::layer()
                .json()
                .flatten_event(true)
                .with_filter(filter)
                .boxed(),
        })
    }

    /// The user's log filter, with OpenTelemetry's internal diagnostics
    /// turned off unless the filter names them explicitly.
    ///
    /// The request span is hidden from log outputs: it exists for tracing,
    /// and the access-log event already carries the same fields.
    pub(crate) fn log_filter(level: &str) -> Result<EnvFilter> {
        let mut directives = format!("{level},{HTTP_SPAN_TARGET}=off");
        if !level.contains("opentelemetry") {
            for target in OTEL_INTERNAL_TARGETS {
                directives.push_str(&format!(",{target}=off"));
            }
        }
        Ok(EnvFilter::builder().parse(directives)?)
    }
}
