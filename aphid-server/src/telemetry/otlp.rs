use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use opentelemetry::{Key, KeyValue, global, trace::TracerProvider as _};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
    logs::{LogBatch, LogExporter, SdkLoggerProvider},
    propagation::TraceContextPropagator,
    resource::{EnvResourceDetector, ResourceDetector, TelemetryResourceDetector},
    trace::{SdkTracerProvider, SpanData, SpanExporter},
};
use tracing::{Level, Subscriber};
use tracing_subscriber::{
    Layer,
    filter::{FilterExt, Targets, filter_fn},
    registry::LookupSpan,
};

use super::Telemetry;
#[cfg(test)]
use crate::access_log::ACCESS_LOG_TARGET;
use crate::{Result, access_log::HTTP_SPAN_TARGET};

const DEFAULT_SERVICE_NAME: &str = "aphid-server";
const DEFAULT_EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// Targets whose events must never become exported log records: the
/// exporter's own HTTP stack and the SDK. Exporting them would feed back
/// into the exporter.
const NOT_EXPORTED_TARGETS: &[&str] = &[
    "opentelemetry",
    "tracing_opentelemetry",
    "reqwest",
    "hyper",
    "hyper_util",
    "h2",
    "rustls",
    "tower",
];

/// Tracer and logger providers exporting over OTLP/HTTP with protobuf.
///
/// Both use the SDK's batch processors: spans and log records are queued in
/// memory and exported from background threads, so a slow or unreachable
/// collector never blocks a request (a full queue drops data instead).
pub(crate) struct OtlpPipeline {
    tracer_provider: SdkTracerProvider,
    logger_provider: SdkLoggerProvider,
}

impl OtlpPipeline {
    /// Build OTLP/HTTP exporters for `endpoint`, a base URL like
    /// `OTEL_EXPORTER_OTLP_ENDPOINT` (`/v1/traces` and `/v1/logs` are
    /// appended). Signal-specific `OTEL_EXPORTER_OTLP_{TRACES,LOGS}_ENDPOINT`
    /// variables still win. Headers, timeouts and resource attributes come
    /// from the standard `OTEL_*` variables.
    pub(crate) fn from_endpoint(endpoint: &str) -> Result<Self> {
        let client = Self::http_client()?;
        let spans = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_http_client(client.clone())
            .with_protocol(Protocol::HttpBinary);
        let spans = match Self::signal_endpoint(endpoint, "TRACES", "v1/traces") {
            Some(url) => spans.with_endpoint(url),
            None => spans,
        }
        .build()?;

        let logs = opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_http_client(client)
            .with_protocol(Protocol::HttpBinary);
        let logs = match Self::signal_endpoint(endpoint, "LOGS", "v1/logs") {
            Some(url) => logs.with_endpoint(url),
            None => logs,
        }
        .build()?;

        Ok(Self::new(
            ReportFailures::new(spans, "traces"),
            ReportFailures::new(logs, "logs"),
            Self::resource(),
        ))
    }

    pub(crate) fn new(
        spans: impl SpanExporter + 'static,
        logs: impl LogExporter + 'static,
        resource: Resource,
    ) -> Self {
        global::set_text_map_propagator(TraceContextPropagator::new());
        Self {
            tracer_provider: SdkTracerProvider::builder()
                .with_batch_exporter(spans)
                .with_resource(resource.clone())
                .build(),
            logger_provider: SdkLoggerProvider::builder()
                .with_batch_exporter(logs)
                .with_resource(resource)
                .build(),
        }
    }

    /// The tracing layers that feed this pipeline:
    /// - request spans (target [`HTTP_SPAN_TARGET`]) become OTel server spans,
    ///   whatever the log level;
    /// - events passing the log filter become OTel log records, correlated
    ///   with the active span.
    pub(crate) fn layers<S>(&self, log_level: &str) -> Result<impl Layer<S> + use<S>>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let tracer = self.tracer_provider.tracer(env!("CARGO_PKG_NAME"));
        let spans = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_tracked_inactivity(false)
            .with_threads(false)
            .with_location(false)
            .with_target(false)
            .with_filter(Targets::new().with_target(HTTP_SPAN_TARGET, Level::INFO));

        let exportable = filter_fn(|meta| {
            !NOT_EXPORTED_TARGETS
                .iter()
                .any(|t| meta.target().starts_with(t))
        });
        let logs = OpenTelemetryTracingBridge::new(&self.logger_provider)
            .with_filter(Telemetry::log_filter(log_level)?.and(exportable));

        Ok(spans.and_then(logs))
    }

    #[cfg(test)]
    pub(crate) fn force_flush(&self) {
        self.tracer_provider.force_flush().unwrap();
        self.logger_provider.force_flush().unwrap();
    }

    /// Flush and shut down both providers in parallel, each bounded by
    /// `timeout`.
    pub(crate) fn shutdown(self, timeout: Duration) {
        let (traces, logs) = std::thread::scope(|s| {
            let traces = s.spawn(|| self.tracer_provider.shutdown_with_timeout(timeout));
            let logs = self.logger_provider.shutdown_with_timeout(timeout);
            (traces.join(), logs)
        });
        if !matches!(traces, Ok(Ok(()))) || logs.is_err() {
            tracing::warn!(
                target: "aphid_server::telemetry",
                "OpenTelemetry flush on shutdown incomplete; some telemetry was dropped"
            );
        }
    }

    /// The blocking HTTP client both exporters share. HTTPS endpoints are
    /// verified against Mozilla's root store compiled into the binary
    /// (`webpki-root-certs`), so the `scratch` image needs no CA bundle.
    /// The timeout mirrors the SDK default: `OTEL_EXPORTER_OTLP_TIMEOUT`
    /// milliseconds, else 10 s.
    ///
    /// Must be built outside the tokio runtime (see [`Telemetry::init`]).
    fn http_client() -> Result<reqwest::blocking::Client> {
        let timeout = std::env::var("OTEL_EXPORTER_OTLP_TIMEOUT")
            .ok()
            .and_then(|ms| ms.trim().parse().ok())
            .map_or(DEFAULT_EXPORT_TIMEOUT, Duration::from_millis);
        let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .map(|der| reqwest::Certificate::from_der(der))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(reqwest::blocking::Client::builder()
            .timeout(timeout)
            .tls_certs_only(roots)
            .build()?)
    }

    fn signal_endpoint(base: &str, signal: &str, path: &str) -> Option<String> {
        let signal_var = format!("OTEL_EXPORTER_OTLP_{signal}_ENDPOINT");
        if std::env::var(signal_var).is_ok_and(|v| !v.is_empty()) {
            return None;
        }
        Some(format!("{}/{path}", base.trim_end_matches('/')))
    }

    /// `service.name` from `OTEL_SERVICE_NAME`, then `OTEL_RESOURCE_ATTRIBUTES`,
    /// then `aphid-server`; other attributes from `OTEL_RESOURCE_ATTRIBUTES`.
    fn resource() -> Resource {
        let service_name = std::env::var("OTEL_SERVICE_NAME")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                EnvResourceDetector::new()
                    .detect()
                    .get(&Key::from_static_str("service.name"))
                    .map(|v| v.to_string())
            })
            .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_owned());

        Resource::builder_empty()
            .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
            .with_detectors(&[
                Box::new(TelemetryResourceDetector),
                Box::new(EnvResourceDetector::new()),
            ])
            .with_service_name(service_name)
            .build()
    }
}

/// Exporter wrapper that reports a failing endpoint once, and its recovery
/// once, instead of on every export attempt.
#[derive(Debug)]
pub(crate) struct ReportFailures<E> {
    inner: E,
    signal: &'static str,
    failing: AtomicBool,
}

impl<E> ReportFailures<E> {
    pub(crate) fn new(inner: E, signal: &'static str) -> Self {
        Self {
            inner,
            signal,
            failing: AtomicBool::new(false),
        }
    }

    fn observe(&self, result: &OTelSdkResult) {
        match result {
            Err(error) if !self.failing.swap(true, Ordering::Relaxed) => tracing::warn!(
                target: "aphid_server::telemetry",
                signal = self.signal,
                %error,
                "OTLP export failing; further failures are not logged until it recovers"
            ),
            Ok(()) if self.failing.swap(false, Ordering::Relaxed) => tracing::info!(
                target: "aphid_server::telemetry",
                signal = self.signal,
                "OTLP export recovered"
            ),
            _ => {}
        }
    }
}

impl<E: SpanExporter> SpanExporter for ReportFailures<E> {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let result = self.inner.export(batch).await;
        self.observe(&result);
        result
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
}

impl<E: LogExporter> LogExporter for ReportFailures<E> {
    async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
        let result = self.inner.export(batch).await;
        self.observe(&result);
        result
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn event_enabled(
        &self,
        level: opentelemetry::logs::Severity,
        target: &str,
        name: Option<&str>,
    ) -> bool {
        self.inner.event_enabled(level, target, name)
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
}

#[cfg(test)]
mod tests;
