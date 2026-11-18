//! Runs in its own test binary, because it replaces the process-wide global meter provider.

use axum::{Router, body::Body, extract::Request, routing::get};
use opentelemetry_sdk::metrics::{
    InMemoryMetricExporter, PeriodicReader, SdkMeterProvider, data::ScopeMetrics,
};
use stackable_telemetry::AxumTraceLayer;
use tower::ServiceExt;

#[tokio::test]
async fn layer_created_before_the_meter_provider_records_once_applied() {
    // The layer is created first, as a program does when it configures it before telemetry is
    // initialized. Only applying it to the router resolves the global meter provider.
    let trace_layer = AxumTraceLayer::new();

    let exporter = InMemoryMetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(exporter.clone()).build())
        .build();
    opentelemetry::global::set_meter_provider(provider.clone());

    let router = Router::new()
        .route("/", get(|| async { "hello" }))
        .layer(trace_layer);
    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("valid request");
    router.oneshot(request).await.expect("router is infallible");

    provider.force_flush().expect("in-memory flush succeeds");
    let collections = exporter
        .get_finished_metrics()
        .expect("in-memory exporter is readable");
    let scope_metrics: Vec<_> = collections
        .last()
        .expect("one collection happened")
        .scope_metrics()
        .collect();
    let scope_metrics: &ScopeMetrics = scope_metrics.first().expect("one scope recorded");
    assert_eq!(scope_metrics.scope().name(), "stackable-telemetry");
    assert_eq!(
        scope_metrics.scope().version(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert!(
        scope_metrics
            .metrics()
            .any(|metric| metric.name() == "http.server.request.duration")
    );
}
