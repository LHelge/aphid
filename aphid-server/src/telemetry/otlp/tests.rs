use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{body::Body, http::Request};
use opentelemetry::{Value, trace::SpanKind};
use opentelemetry_sdk::{
    error::OTelSdkError,
    logs::InMemoryLogExporter,
    trace::{InMemorySpanExporter, SpanData},
};
use opentelemetry_semantic_conventions::attribute as semconv;
use tower::ServiceExt;
use tracing_subscriber::{layer::SubscriberExt, registry};

use super::*;
use crate::Server;

const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const PARENT_SPAN_ID: &str = "00f067aa0ba902b7";

fn attributes(span: &SpanData) -> HashMap<String, Value> {
    span.attributes
        .iter()
        .map(|kv| (kv.key.to_string(), kv.value.clone()))
        .collect()
}

#[tokio::test]
async fn exports_server_span_and_correlated_log() {
    let spans = InMemorySpanExporter::default();
    let logs = InMemoryLogExporter::default();
    let pipeline = OtlpPipeline::new(spans.clone(), logs.clone(), OtlpPipeline::resource());
    let subscriber = registry().with(pipeline.layers::<_>("info").unwrap());
    let _guard = tracing::subscriber::set_default(subscriber);

    let root = tempfile::TempDir::new().unwrap();
    let router = Server::router(root.path());
    let request = Request::get("/missing?ref=x")
        .header("traceparent", format!("00-{TRACE_ID}-{PARENT_SPAN_ID}-01"))
        .header("x-forwarded-for", "203.0.113.7")
        .header("user-agent", "test-agent/1.0")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 404);

    let healthz = Request::get("/healthz").body(Body::empty()).unwrap();
    router.oneshot(healthz).await.unwrap();

    pipeline.force_flush();

    let finished = spans.get_finished_spans().unwrap();
    assert_eq!(finished.len(), 1, "one span, none for /healthz");
    let span = &finished[0];
    assert_eq!(span.name, "GET");
    assert_eq!(span.span_kind, SpanKind::Server);
    assert_eq!(span.span_context.trace_id().to_string(), TRACE_ID);
    assert_eq!(span.parent_span_id.to_string(), PARENT_SPAN_ID);
    assert!(span.parent_span_is_remote);

    let attrs = attributes(span);
    let expect = |key: &str, value: Value| {
        assert_eq!(attrs.get(key), Some(&value), "attribute {key}: {attrs:?}");
    };
    expect(semconv::HTTP_REQUEST_METHOD, "GET".into());
    expect(semconv::HTTP_RESPONSE_STATUS_CODE, 404_i64.into());
    expect(semconv::URL_PATH, "/missing".into());
    expect(semconv::URL_QUERY, "ref=x".into());
    expect(semconv::URL_SCHEME, "http".into());
    expect(semconv::NETWORK_PROTOCOL_VERSION, "1.1".into());
    expect(semconv::CLIENT_ADDRESS, "203.0.113.7".into());
    expect(semconv::USER_AGENT_ORIGINAL, "test-agent/1.0".into());
    expect(semconv::HTTP_RESPONSE_BODY_SIZE, 14_i64.into()); // "404 Not Found\n"

    let emitted = logs.get_emitted_logs().unwrap();
    let access = emitted
        .iter()
        .find(|l| l.record.target().is_some_and(|t| t == ACCESS_LOG_TARGET))
        .expect("access log exported as a log record");
    let cx = access
        .record
        .trace_context()
        .expect("log correlated with span");
    assert_eq!(cx.trace_id, span.span_context.trace_id());
    assert_eq!(cx.span_id, span.span_context.span_id());
}

#[test]
fn default_service_name() {
    // Only meaningful when the environment does not set one.
    if std::env::var_os("OTEL_SERVICE_NAME").is_none()
        && std::env::var_os("OTEL_RESOURCE_ATTRIBUTES").is_none()
    {
        let resource = OtlpPipeline::resource();
        let name = resource.get(&Key::from_static_str("service.name"));
        assert_eq!(name, Some(Value::from(DEFAULT_SERVICE_NAME)));
    }
}

#[test]
fn signal_endpoint_appends_path() {
    if std::env::var_os("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT").is_none() {
        assert_eq!(
            OtlpPipeline::signal_endpoint("http://collector:4318/", "TRACES", "v1/traces")
                .as_deref(),
            Some("http://collector:4318/v1/traces")
        );
    }
}

/// Counts `aphid_server::telemetry` events.
struct CountEvents(Arc<AtomicUsize>);

impl<S: Subscriber> Layer<S> for CountEvents {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().target() == "aphid_server::telemetry" {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[test]
fn failures_are_reported_once_per_outage() {
    let count = Arc::new(AtomicUsize::new(0));
    let subscriber = registry().with(CountEvents(Arc::clone(&count)));
    let _guard = tracing::subscriber::set_default(subscriber);

    let reporter = ReportFailures::new((), "traces");
    let fail = || Err(OTelSdkError::InternalFailure("connection refused".into()));
    for _ in 0..5 {
        reporter.observe(&fail());
    }
    assert_eq!(count.load(Ordering::Relaxed), 1, "one warning per outage");

    reporter.observe(&Ok(()));
    reporter.observe(&Ok(()));
    assert_eq!(count.load(Ordering::Relaxed), 2, "one recovery notice");

    reporter.observe(&fail());
    assert_eq!(
        count.load(Ordering::Relaxed),
        3,
        "next outage reported again"
    );
}

#[test]
fn http_client_builds_with_embedded_roots() {
    assert!(!webpki_root_certs::TLS_SERVER_ROOT_CERTS.is_empty());
    OtlpPipeline::http_client().expect("client with webpki roots");
}
