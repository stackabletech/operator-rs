//! Runs in its own test binary, because initializing the metric exporter replaces the
//! process-wide global meter provider.

use stackable_telemetry::Tracing;

// The SDK offers no way to read back the installed global meter provider, so this only checks that
// initialization and shutdown succeed with the metric exporter enabled.
#[tokio::test(flavor = "multi_thread")]
async fn init_and_shutdown_with_metric_exporter_succeed() {
    let tracing_guard = Tracing::builder()
        .service_name("test")
        .with_otlp_metric_exporter(true)
        .build()
        .init()
        .expect("metric exporter builds without a reachable collector");

    opentelemetry::global::meter("test")
        .u64_counter("requests")
        .build()
        .add(1, &[]);

    drop(tracing_guard);
}
