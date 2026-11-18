//! Runs in its own test binary, because it installs a process-wide tracing subscriber: `tracing`
//! caches per-callsite filtering globally, so a thread-local subscriber misses spans when other
//! tests run in parallel.

use axum::{Router, body::Body, extract::Request, http::Version, routing::get};
use opentelemetry::{KeyValue, trace::TracerProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use opentelemetry_semantic_conventions::trace::{
    HTTP_REQUEST_METHOD, HTTP_REQUEST_METHOD_ORIGINAL, HTTP_ROUTE, NETWORK_PROTOCOL_VERSION,
    URL_SCHEME,
};
use stackable_telemetry::AxumTraceLayer;
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;

struct ExpectedSpan {
    method: &'static str,
    name: &'static str,
    recorded_method: &'static str,
    method_original: Option<&'static str>,
}

#[tokio::test]
async fn spans_follow_the_conventions() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    tracing::subscriber::set_global_default(subscriber).expect("no other subscriber is installed");

    let router = Router::new()
        .route("/objects/{id}", get(|| async { "object" }))
        .layer(AxumTraceLayer::new().with_url_scheme("https"));

    let cases = [
        ExpectedSpan {
            method: "GET",
            name: "GET /objects/{id}",
            recorded_method: "GET",
            method_original: None,
        },
        ExpectedSpan {
            method: "BREW",
            name: "HTTP /objects/{id}",
            recorded_method: "_OTHER",
            method_original: Some("BREW"),
        },
    ];

    for case in cases {
        exporter.reset();
        // The bogus scheme of the absolute-form URI must not reach `url.scheme`.
        let request = Request::builder()
            .method(case.method)
            .uri("foo://example.com/objects/42")
            .version(Version::HTTP_11)
            .body(Body::empty())
            .expect("valid request");
        router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        provider.force_flush().expect("in-memory flush succeeds");

        let spans = exporter
            .get_finished_spans()
            .expect("in-memory exporter is readable");
        assert_eq!(spans.len(), 1, "{}: {spans:?}", case.method);
        let span = &spans[0];
        let attribute = |key: &str| {
            span.attributes
                .iter()
                .find(|kv: &&KeyValue| kv.key.as_str() == key)
                .map(|kv| kv.value.to_string())
        };
        assert_eq!(span.name, case.name);
        assert_eq!(
            attribute(HTTP_REQUEST_METHOD).as_deref(),
            Some(case.recorded_method)
        );
        assert_eq!(
            attribute(HTTP_REQUEST_METHOD_ORIGINAL).as_deref(),
            case.method_original
        );
        assert_eq!(attribute(URL_SCHEME).as_deref(), Some("https"));
        assert_eq!(attribute(NETWORK_PROTOCOL_VERSION).as_deref(), Some("1.1"));
        assert_eq!(attribute(HTTP_ROUTE).as_deref(), Some("/objects/{id}"));
    }
}
