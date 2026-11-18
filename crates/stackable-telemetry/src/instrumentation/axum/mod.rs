//! This module contains types which can be used as [`axum`] layers to produce
//! [OpenTelemetry][1] compatible [HTTP spans][2].
//!
//! The same layer records the `http.server.request.duration` metric defined by
//! the [HTTP metric conventions][3]. Spans and metric take the method, scheme,
//! route and protocol version from the same [`attributes::ServerRequestAttributes`].
//!
//! The spans include a wide variety of fields / attributes defined by the
//! semantic conventions specification. A few examples are:
//!
//! - `http.request.method`
//! - `http.response.status_code`
//! - `user_agent.original`
//!
//! [1]: https://opentelemetry.io/
//! [2]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/
//! [3]: https://opentelemetry.io/docs/specs/semconv/http/http-metrics/
use std::{future::Future, net::SocketAddr, num::ParseIntError, task::Poll, time::Instant};

use axum::{
    extract::{ConnectInfo, MatchedPath, Request},
    http::{
        HeaderMap,
        header::{HOST, USER_AGENT},
    },
    response::Response,
};
use futures_util::ready;
use opentelemetry::{
    Context, InstrumentationScope,
    metrics::{Histogram, Meter},
    trace::{SpanKind, TraceContextExt},
};
use opentelemetry_semantic_conventions as semconv;
use opentelemetry_semantic_conventions::trace::HTTP_REQUEST_HEADER;
use pin_project::{pin_project, pinned_drop};
use snafu::{ResultExt, Snafu};
use tower::{Layer, Service};
use tracing::{Span, field::Empty};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod attributes;
mod extractor;
mod injector;

pub use extractor::*;
pub use injector::*;

const X_FORWARDED_HOST_HEADER_KEY: &str = "X-Forwarded-Host";
const DEFAULT_HTTPS_PORT: u16 = 443;
const DEFAULT_HTTP_PORT: u16 = 80;

// NOTE (@Techassi): These constants are defined here because they are private in
// the tracing-opentelemetry crate.
const OTEL_NAME: &str = "otel.name";
const OTEL_KIND: &str = "otel.kind";

const OTEL_TRACE_ID_FROM: &str = "opentelemetry.trace_id.from";
const OTEL_TRACE_ID_TO: &str = "opentelemetry.trace_id.to";

const METER_NAME: &str = "stackable-telemetry";
const DEFAULT_URL_SCHEME: &str = "http";

/// The bucket boundaries the HTTP metric conventions advise for `http.server.request.duration`.
const REQUEST_DURATION_BOUNDARIES: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

/// A Tower [`Layer`][1] which decorates [`TraceService`].
///
/// Besides the span, every request is recorded in the `http.server.request.duration` histogram,
/// at the cost of one histogram measurement and a few attribute allocations per request.
///
/// Unless [`TraceLayer::with_meter`] provides a meter, the layer takes its histogram from the
/// global meter provider when it is applied, which for axum is when the `Router` is built. Build
/// the router after [`Tracing::init`](crate::Tracing::init): before that, the global provider is
/// a no-op, and a histogram taken from it records nothing for the lifetime of the router.
///
/// The duration ends when the inner service has produced the response head, not when the body
/// has been sent. A request whose future is dropped before that, for example because the client
/// or the Kubernetes API server timed out, is recorded with `error.type` `cancelled` and no status
/// code, so slow requests that never finish still appear in the histogram.
///
/// ### Example with Axum
///
/// ```
/// use axum::{Router, routing::get};
/// use stackable_telemetry::AxumTraceLayer;
///
/// let trace_layer = AxumTraceLayer::new();
/// let router = Router::new()
///     .route("/", get(|| async { "Hello, World!" }))
///     .layer(trace_layer);
///
/// # let _: Router = router;
/// ```
///
/// This layer is implemented based on [this][1] official Tower guide.
///
/// [1]: https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md
#[derive(Clone, Debug)]
pub struct TraceLayer {
    opt_in: bool,
    url_scheme: &'static str,
    meter: Option<Meter>,
}

impl Default for TraceLayer {
    fn default() -> Self {
        Self {
            opt_in: false,
            url_scheme: DEFAULT_URL_SCHEME,
            meter: None,
        }
    }
}

impl<S> Layer<S> for TraceLayer {
    type Service = TraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        let meter = self.meter.clone().unwrap_or_else(|| {
            opentelemetry::global::meter_with_scope(
                InstrumentationScope::builder(METER_NAME)
                    .with_version(env!("CARGO_PKG_VERSION"))
                    .build(),
            )
        });

        TraceService {
            inner,
            opt_in: self.opt_in,
            url_scheme: self.url_scheme,
            request_duration: request_duration_histogram(&meter),
        }
    }
}

impl TraceLayer {
    /// Creates a new default trace layer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the request duration through `meter` instead of the global meter provider.
    pub fn with_meter(mut self, meter: Meter) -> Self {
        self.meter = Some(meter);
        self
    }

    /// Sets the `url.scheme` that spans and metric record. Defaults to `http`. A server terminating
    /// TLS sets `https`. The scheme in the request URI is never used, because the client controls
    /// it.
    pub fn with_url_scheme(mut self, url_scheme: &'static str) -> Self {
        self.url_scheme = url_scheme;
        self
    }

    /// Enables various fields marked as opt-in by the specification.
    ///
    /// This will require more computing power and will increase the latency.
    /// See <https://opentelemetry.io/docs/specs/semconv/http/http-spans/>
    pub fn with_opt_in(mut self) -> Self {
        self.opt_in = true;
        self
    }
}

fn request_duration_histogram(meter: &Meter) -> Histogram<f64> {
    meter
        .f64_histogram(opentelemetry_semantic_conventions::metric::HTTP_SERVER_REQUEST_DURATION)
        .with_unit("s")
        .with_description("Duration of HTTP server requests.")
        .with_boundaries(REQUEST_DURATION_BOUNDARIES.to_vec())
        .build()
}

/// A Tower [`Service`] which injects Span Context into HTTP Response Headers and records the
/// request duration.
#[derive(Debug, Clone)]
pub struct TraceService<S> {
    inner: S,
    opt_in: bool,
    url_scheme: &'static str,
    request_duration: Histogram<f64>,
}

impl<S> Service<Request> for TraceService<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Error: std::error::Error + 'static,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;
    type Response = S::Response;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let started = Instant::now();
        let request_attributes =
            attributes::ServerRequestAttributes::from_request(&req, self.url_scheme);
        let span = Span::from_request(&req, &request_attributes, self.opt_in);

        let future = {
            let _guard = span.enter();
            self.inner.call(req)
        };

        ResponseFuture {
            future,
            span,
            request_duration: self.request_duration.clone(),
            request_attributes: Some(request_attributes),
            started,
        }
    }
}

/// This future contains the inner service future and the current [`Span`].
#[pin_project(PinnedDrop)]
pub struct ResponseFuture<F> {
    #[pin]
    pub(crate) future: F,
    pub(crate) span: Span,
    pub(crate) request_duration: Histogram<f64>,
    // Taken when the duration is recorded, either when the response is ready or when the future is
    // dropped before that, so it is recorded exactly once.
    pub(crate) request_attributes: Option<attributes::ServerRequestAttributes>,
    pub(crate) started: Instant,
}

impl<F, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, E>>,
    E: std::error::Error + 'static,
{
    type Output = Result<Response, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        let _guard = this.span.enter();

        let mut result = ready!(this.future.poll(cx));
        this.span.finalize(&mut result);

        if let Some(request_attributes) = this.request_attributes.take() {
            let outcome = match &result {
                // Every response is `Ok`, including 4xx and 5xx. `into_key_values` derives
                // `error.type` from a 5xx status.
                Ok(response) => attributes::RequestOutcome::Response(response.status()),
                Err(_) => attributes::RequestOutcome::Failed(std::any::type_name::<E>()),
            };
            let key_values = request_attributes.into_key_values(outcome);
            this.request_duration
                .record(this.started.elapsed().as_secs_f64(), &key_values);
        }

        Poll::Ready(result)
    }
}

#[pinned_drop]
impl<F> PinnedDrop for ResponseFuture<F> {
    fn drop(self: std::pin::Pin<&mut Self>) {
        let this = self.project();
        if let Some(request_attributes) = this.request_attributes.take() {
            let key_values =
                request_attributes.into_key_values(attributes::RequestOutcome::Cancelled);
            this.request_duration
                .record(this.started.elapsed().as_secs_f64(), &key_values);
        }
    }
}

/// Errors which can be encountered when extracting the server host from a [`Request`].
#[derive(Debug, Snafu)]
pub enum ServerHostError {
    /// Indicates that parsing the port of the server host from the [`Request`] as a `u16` failed.
    #[snafu(display("failed to parse port {port:?} as u16 from string"))]
    ParsePort {
        #[allow(missing_docs)]
        source: ParseIntError,

        // TODO (@Techassi): Make snafu re-emit this
        /// The original input which was attempted to be parsed.
        port: String,
    },

    /// Indicates that the server host from the [`Request`] contains an invalid/unknown scheme.
    #[snafu(display("encountered invalid request scheme {scheme:?}"))]
    InvalidScheme {
        /// The original scheme.
        scheme: String,
    },

    // TODO (@Techassi): Make snafu re-emit this
    /// Indicates that no method of extracting the server host from the [`Request`] succeeded.
    #[snafu(display("failed to extract any host information from request"))]
    ExtractHost,
}

/// This trait provides various helper functions to extract data from a HTTP [`Request`].
pub trait RequestExt {
    /// Returns the client socket address, if available.
    fn client_socket_address(&self) -> Option<SocketAddr>;

    /// Returns the server host, if available.
    ///
    /// ### Value Selection Strategy
    ///
    /// The following value selection strategy is taken verbatim from [this][1]
    /// section of the HTTP span semantic conventions:
    ///
    /// > HTTP server instrumentations SHOULD do the best effort when populating
    /// > server.address and server.port attributes and SHOULD determine them by
    /// > using the first of the following that applies:
    /// >
    /// > - The original host which may be passed by the reverse proxy in the
    /// >  Forwarded#host, X-Forwarded-Host, or a similar header.
    /// > - The :authority pseudo-header in case of HTTP/2 or HTTP/3
    /// > - The Host header.
    ///
    /// [1]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes
    fn server_host(&self) -> Result<(String, u16), ServerHostError>;

    /// Returns the matched path, like `/object/:object_id/tags`.
    ///
    /// The returned path has low cardinality. It will never contain any path
    /// or query parameter. This behaviour is suggested by the conventions
    /// specification.
    fn matched_path(&self) -> Option<&MatchedPath>;

    /// Returns the span name.
    ///
    /// The format is either `{method} {http.route}` or `{method}` if
    /// `http.route` is not available, where a method the conventions do not
    /// list as known is written as `HTTP`. Examples are:
    ///
    /// - `GET /object/:object_id/tags`
    /// - `PUT /upload/:file_id`
    /// - `POST /convert`
    /// - `OPTIONS`
    /// - `HTTP`
    fn span_name(&self) -> String;

    /// Returns the user agent, if available.
    fn user_agent(&self) -> Option<&str>;
}

impl RequestExt for Request {
    fn server_host(&self) -> Result<(String, u16), ServerHostError> {
        // There is currently no obvious way to use the Host extractor from Axum
        // directly. Using that extractor either requires impossible code (async
        // in the Service's call function, unnecessary cloning or consuming self
        // and returning a newly created request). That's why the following
        // section mirrors the Axum extractor implementation. The implementation
        // currently only looks for the X-Forwarded-Host / Host header and falls
        // back to the request URI host. The Axum implementation also extracts
        // data from the Forwarded header.

        if let Some(host) = self
            .headers()
            .get(X_FORWARDED_HOST_HEADER_KEY)
            .and_then(|host| host.to_str().ok())
        {
            return server_host_to_tuple(host, self.uri().scheme_str());
        }

        if let Some(host) = self.headers().get(HOST).and_then(|host| host.to_str().ok()) {
            return server_host_to_tuple(host, self.uri().scheme_str());
        }

        if let (Some(host), Some(port)) = (self.uri().host(), self.uri().port_u16()) {
            return Ok((host.to_owned(), port));
        }

        ExtractHostSnafu.fail()
    }

    fn client_socket_address(&self) -> Option<SocketAddr> {
        self.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0)
    }

    fn matched_path(&self) -> Option<&MatchedPath> {
        self.extensions().get::<MatchedPath>()
    }

    fn span_name(&self) -> String {
        attributes::span_name(
            attributes::http_request_method(self.method()),
            attributes::http_route(self),
        )
    }

    fn user_agent(&self) -> Option<&str> {
        self.headers()
            .get(USER_AGENT)
            .map(|ua| ua.to_str().unwrap_or_default())
    }
}

fn server_host_to_tuple(
    host: &str,
    scheme: Option<&str>,
) -> Result<(String, u16), ServerHostError> {
    if let Some((host, port)) = host.split_once(':') {
        // First, see if the host header value contains a colon indicating that
        // it includes a non-default port.
        let port: u16 = port.parse().context(ParsePortSnafu { port })?;
        Ok((host.to_owned(), port))
    } else {
        // If there is no port included in the header value, the port is implied.
        // Port 443 for HTTPS and port 80 for HTTP.
        let port = match scheme {
            Some("https") => DEFAULT_HTTPS_PORT,
            Some("http") => DEFAULT_HTTP_PORT,
            Some(scheme) => return InvalidSchemeSnafu { scheme }.fail(),
            _ => return InvalidSchemeSnafu { scheme: "" }.fail(),
        };

        Ok((host.to_owned(), port))
    }
}

/// This trait provides various helper functions to create a [`Span`] out of
/// an HTTP [`Request`].
pub trait SpanExt {
    /// Create a span according to the semantic conventions for HTTP spans from
    /// an Axum [`Request`].
    ///
    /// The individual fields are defined in [this specification][1]. Some of
    /// them are:
    ///
    /// - `http.request.method`
    /// - `http.response.status_code`
    /// - `network.protocol.version`
    ///
    /// The method, scheme, route and protocol version come from `attributes`,
    /// so that the span has the same values as the request duration metric.
    ///
    /// Setting the `opt_in` parameter to `true` enables various fields marked
    /// as opt-in by the specification. This will require more computing power
    /// and will increase the latency.
    ///
    /// [1]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/
    fn from_request(
        req: &Request,
        attributes: &attributes::ServerRequestAttributes,
        opt_in: bool,
    ) -> Self;

    /// Adds HTTP request headers to the span as a `http.request.header.<key>`
    /// field.
    ///
    /// NOTE: This is currently not supported, because [`tracing`] doesn't
    /// support recording dynamic fields.
    fn add_header_fields(&self, headers: &HeaderMap);

    /// Finalize the [`Span`] with an Axum [`Response`].
    fn finalize_with_response(&self, response: &mut Response);

    /// Finalize the [`Span`] with an error.
    fn finalize_with_error<E>(&self, error: &mut E)
    where
        E: std::error::Error;

    /// Finalize the [`Span`] with a result.
    ///
    /// The default implementation internally calls:
    ///
    /// - [`SpanExt::finalize_with_response`] when [`Ok`]
    /// - [`SpanExt::finalize_with_error`] when [`Err`]
    fn finalize<E>(&self, result: &mut Result<Response, E>)
    where
        E: std::error::Error,
    {
        match result {
            Ok(response) => self.finalize_with_response(response),
            Err(error) => self.finalize_with_error(error),
        }
    }
}

impl SpanExt for Span {
    fn from_request(
        req: &Request,
        attributes: &attributes::ServerRequestAttributes,
        opt_in: bool,
    ) -> Self {
        let http_method = attributes.method;
        let span_name = attributes::span_name(http_method, attributes.route.as_deref());
        let url = req.uri();

        tracing::trace!(
            http_method,
            span_name,
            ?url,
            "extracted http method, span name and request url"
        );

        // The span name follows the format `{method} {http.route}` defined
        // by the semantic conventions spec from the OpenTelemetry project.
        // Currently, the tracing crate doesn't allow non 'static span names,
        // and thus, the special field otel.name is used to set the span name.
        // The span name defined in the trace_span macro only serves as a
        // placeholder.
        //
        // - https://docs.rs/tracing-opentelemetry/latest/tracing_opentelemetry/#special-fields
        // - https://github.com/tokio-rs/tracing/issues/1047
        // - https://github.com/tokio-rs/tracing/pull/732
        //
        // Additionally we cannot use consts for field names. There was an
        // upstream PR to add support for it, but it was unexpectedly closed.
        // See https://github.com/tokio-rs/tracing/pull/2254.
        //
        // If this is eventually supported (maybe with our efforts), we can use
        // the opentelemetry-semantic-conventions crate, see here:
        // https://docs.rs/opentelemetry-semantic-conventions/latest/opentelemetry_semantic_conventions/index.html

        // Setting common fields first
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#common-attributes

        let span = tracing::trace_span!(
            "HTTP request",
            { OTEL_NAME } = span_name,
            { OTEL_KIND } = ?SpanKind::Server,
            { semconv::attribute::OTEL_STATUS_CODE } = Empty,
            // The current tracing-opentelemetry version still uses the old semantic convention
            // See https://github.com/tokio-rs/tracing-opentelemetry/pull/209
            { semconv::attribute::OTEL_STATUS_DESCRIPTION } = Empty,
            { semconv::trace::HTTP_REQUEST_METHOD } = http_method,
            { semconv::trace::HTTP_REQUEST_METHOD_ORIGINAL } = Empty,
            { semconv::trace::HTTP_RESPONSE_STATUS_CODE } = Empty,
            { semconv::trace::HTTP_ROUTE } = attributes.route.as_deref(),
            { semconv::trace::URL_PATH } = url.path(),
            { semconv::trace::URL_QUERY } = url.query(),
            { semconv::trace::URL_SCHEME } = attributes.scheme,
            { semconv::trace::NETWORK_PROTOCOL_VERSION } = attributes.protocol_version,
            { semconv::trace::USER_AGENT_ORIGINAL } = Empty,
            { semconv::trace::SERVER_ADDRESS } = Empty,
            { semconv::trace::SERVER_PORT } = Empty,
            { semconv::trace::CLIENT_ADDRESS } = Empty,
            { semconv::trace::CLIENT_PORT } = Empty,
        );

        if http_method == attributes::OTHER_METHOD {
            span.record(
                semconv::trace::HTTP_REQUEST_METHOD_ORIGINAL,
                req.method().as_str(),
            );
        }

        // Set the parent span based on the extracted context
        //
        // The OpenTelemetry spec does not allow trace ids to be updated after
        // a span has been created. Since the (optional) new trace id given by
        // a client is only knowable after handling the request, it is not
        // available to the existing parent spans for the lower layers (tcp/tls
        // handling).
        //
        // Therefore, we have to made a decision about linking the two traces.
        // These are the options:
        // 1. Link to the trace id supplied in the incoming request, or
        // 2. Link to the current trace id, then set the parent context based on
        //    trace information supplied in the incoming request.
        //
        // Neither is ideal, as it means there are (at least) two traces to look
        // at to get a complete picture of what happened over the request.
        //
        // Option 1 is not viable, as the trace id in the response headers will
        // not be the same as what the client sent. Yet we are supposed to pass
        // their trace id in any further requests.
        //
        // We will go with option 2 as it at least keeps the higher layer spans
        // in one trace, which is likely going to be more useful to the person
        // visualizing the traces.
        let new_parent = HeaderExtractor::new(req.headers()).extract_context();
        let new_span_context = new_parent.span().span_context().clone();
        let current_span_context = Context::current().span().span_context().clone();

        if new_span_context != current_span_context {
            tracing::trace!(
                { OTEL_TRACE_ID_FROM } = ?current_span_context.trace_id(),
                { OTEL_TRACE_ID_TO } = ?new_span_context.trace_id(),
                "set parent span context based on context extracted from request headers"
            );

            Self::current().add_link(new_parent.span().span_context().clone());
            span.add_link(Context::current().span().span_context().to_owned());
            let _ = span.set_parent(new_parent);
        }

        if let Some(user_agent) = req.user_agent() {
            span.record(semconv::trace::USER_AGENT_ORIGINAL, user_agent);
        }

        // Setting server.address and server.port
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes

        if let Ok((host, port)) = req.server_host() {
            // NOTE (@Techassi): We cast to i64, because otherwise the field
            // will NOT be recorded as a number but as a string. This is likely
            // an issue in the tracing-opentelemetry crate.
            span.record(semconv::trace::SERVER_ADDRESS, host)
                .record(semconv::trace::SERVER_PORT, i64::from(port));
        }

        // Setting fields according to the HTTP server semantic conventions
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-server-semantic-conventions

        if let Some(client_socket_address) = req.client_socket_address() {
            span.record(
                semconv::trace::CLIENT_ADDRESS,
                client_socket_address.ip().to_string(),
            );

            if opt_in {
                // NOTE (@Techassi): We cast to i64, because otherwise the field
                // will NOT be recorded as a number but as a string. This is
                // likely an issue in the tracing-opentelemetry crate.
                span.record(
                    semconv::trace::CLIENT_PORT,
                    i64::from(client_socket_address.port()),
                );
            }
        }

        // Only include the headers if the user opted in, because this might
        // potentially be an expensive operation when many different headers
        // are present. The OpenTelemetry spec also marks this as opt-in.

        // FIXME (@Techassi): Currently, tracing doesn't support recording
        // fields which are not registered at span creation which thus makes it
        // impossible to record request headers at runtime.
        // See: https://github.com/tokio-rs/tracing/issues/1343

        span
    }

    fn add_header_fields(&self, headers: &HeaderMap) {
        for (header_name, header_value) in headers {
            // TODO (@Techassi): Add an allow list for header names
            // TODO (@Techassi): Handle multiple headers with the same name

            // header_name.as_str() always returns lowercase strings and thus we
            // don't need to call to_lowercase on it.
            let header_name = header_name.as_str();
            let field_name = format!("{HTTP_REQUEST_HEADER}.{header_name}");

            self.record(
                field_name.as_str(),
                header_value.to_str().unwrap_or_default(),
            );
        }
    }

    fn finalize_with_response(&self, response: &mut Response) {
        let status_code = response.status();

        // NOTE (@Techassi): We cast to i64, because otherwise the field will
        // NOT be recorded as a number but as a string. This is likely an issue
        // in the tracing-opentelemetry crate.
        self.record(
            semconv::trace::HTTP_RESPONSE_STATUS_CODE,
            i64::from(status_code.as_u16()),
        );

        // Only set the span status to "Error" when we encountered an server
        // error. See:
        //
        // - https://opentelemetry.io/docs/specs/semconv/http/http-spans/#status
        // - https://github.com/open-telemetry/opentelemetry-specification/blob/v1.26.0/specification/trace/api.md#set-status
        if status_code.is_server_error() {
            self.record(semconv::attribute::OTEL_STATUS_CODE, "Error");
            // NOTE (@Techassi): Can we add a status_description here as well?
        }

        let mut injector = HeaderInjector::new(response.headers_mut());
        injector.inject_context(&Self::current().context());
    }

    fn finalize_with_error<E>(&self, error: &mut E)
    where
        E: std::error::Error,
    {
        // NOTE (@Techassi): This field might get renamed: https://github.com/tokio-rs/tracing-opentelemetry/issues/115
        // NOTE (@Techassi): It got renamed, a fixed version of tracing-opentelemetry is not available yet
        self.record(semconv::attribute::OTEL_STATUS_CODE, "Error")
            .record(
                semconv::attribute::OTEL_STATUS_DESCRIPTION,
                error.to_string(),
            );
    }
}

#[cfg(test)]
mod test {
    use axum::{Router, body::Body, http::StatusCode, routing::get};
    use opentelemetry::{KeyValue, metrics::MeterProvider};
    use opentelemetry_sdk::metrics::{
        InMemoryMetricExporter, PeriodicReader, SdkMeterProvider,
        data::{AggregatedMetrics, MetricData, ScopeMetrics},
    };
    use rstest::rstest;
    use tower::ServiceExt;

    use super::*;

    fn meter_provider() -> (SdkMeterProvider, InMemoryMetricExporter) {
        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_reader(PeriodicReader::builder(exporter.clone()).build())
            .build();
        (provider, exporter)
    }

    /// Returns the attribute sets of all `http.server.request.duration` data points.
    fn recorded_attributes(
        provider: &SdkMeterProvider,
        exporter: &InMemoryMetricExporter,
    ) -> Vec<Vec<KeyValue>> {
        provider.force_flush().expect("in-memory flush succeeds");
        let collections = exporter
            .get_finished_metrics()
            .expect("in-memory exporter is readable");
        let resource = collections.last().expect("one collection happened");
        resource
            .scope_metrics()
            .flat_map(ScopeMetrics::metrics)
            .filter(|metric| {
                metric.name()
                    == opentelemetry_semantic_conventions::metric::HTTP_SERVER_REQUEST_DURATION
            })
            .flat_map(|metric| match metric.data() {
                AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
                    .data_points()
                    .map(|point| point.attributes().cloned().collect())
                    .collect::<Vec<_>>(),
                other => panic!("unexpected metric data: {other:?}"),
            })
            .collect()
    }

    fn value(attributes: &[KeyValue], key: &str) -> Option<String> {
        attributes
            .iter()
            .find(|kv| kv.key.as_str() == key)
            .map(|kv| kv.value.to_string())
    }

    /// How the layer is attached to the router.
    #[derive(Clone, Copy, Debug)]
    enum Attach {
        RouteLayer,
        RouterLayer,
    }

    fn router(trace_layer: TraceLayer, attach: Attach) -> Router {
        let router = Router::new()
            .route("/objects/{id}", get(|| async { "object" }))
            .route(
                "/fail",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "failed") }),
            );
        match attach {
            Attach::RouteLayer => router.route_layer(trace_layer),
            Attach::RouterLayer => router.layer(trace_layer),
        }
    }

    #[rstest]
    #[case::route_layer(
        Attach::RouteLayer,
        "GET",
        "/objects/42",
        Some("/objects/{id}"),
        "GET",
        "200",
        None
    )]
    #[case::router_layer(
        Attach::RouterLayer,
        "GET",
        "/objects/42",
        Some("/objects/{id}"),
        "GET",
        "200",
        None
    )]
    #[case::fallback_has_no_route(Attach::RouterLayer, "GET", "/missing", None, "GET", "404", None)]
    #[case::server_error(
        Attach::RouterLayer,
        "GET",
        "/fail",
        Some("/fail"),
        "GET",
        "500",
        Some("500")
    )]
    #[case::unknown_method(
        Attach::RouterLayer,
        "BREW",
        "/objects/42",
        Some("/objects/{id}"),
        "_OTHER",
        "405",
        None
    )]
    #[tokio::test]
    async fn records_request_duration(
        #[case] attach: Attach,
        #[case] method: &str,
        #[case] uri: &str,
        #[case] expected_route: Option<&str>,
        #[case] expected_method: &str,
        #[case] expected_status: &str,
        #[case] expected_error_type: Option<&str>,
    ) {
        let (provider, exporter) = meter_provider();
        let trace_layer = TraceLayer::new().with_meter(provider.meter("test"));
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .expect("valid request");

        router(trace_layer, attach)
            .oneshot(request)
            .await
            .expect("router is infallible");

        let points = recorded_attributes(&provider, &exporter);
        assert_eq!(points.len(), 1, "{points:?}");
        let attributes = &points[0];
        assert_eq!(
            value(attributes, semconv::attribute::HTTP_ROUTE).as_deref(),
            expected_route
        );
        assert_eq!(
            value(attributes, semconv::attribute::HTTP_REQUEST_METHOD).as_deref(),
            Some(expected_method)
        );
        assert_eq!(
            value(attributes, semconv::attribute::HTTP_RESPONSE_STATUS_CODE).as_deref(),
            Some(expected_status)
        );
        assert_eq!(
            value(attributes, semconv::attribute::ERROR_TYPE).as_deref(),
            expected_error_type
        );
        assert_eq!(
            value(attributes, semconv::attribute::URL_SCHEME).as_deref(),
            Some("http")
        );
    }

    #[tokio::test]
    async fn records_a_request_dropped_before_its_response_as_cancelled() {
        let (provider, exporter) = meter_provider();
        let trace_layer = TraceLayer::new().with_meter(provider.meter("test"));
        let router = Router::new()
            .route("/hang", get(std::future::pending::<&'static str>))
            .layer(trace_layer);
        let request = Request::builder()
            .uri("/hang")
            .body(Body::empty())
            .expect("valid request");

        let timed_out = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            router.oneshot(request),
        )
        .await;
        assert!(timed_out.is_err(), "the handler never completes");

        let points = recorded_attributes(&provider, &exporter);
        assert_eq!(points.len(), 1, "{points:?}");
        let attributes = &points[0];
        assert_eq!(
            value(attributes, semconv::attribute::ERROR_TYPE).as_deref(),
            Some(attributes::CANCELLED_ERROR_TYPE)
        );
        assert_eq!(
            value(attributes, semconv::attribute::HTTP_RESPONSE_STATUS_CODE),
            None
        );
        assert_eq!(
            value(attributes, semconv::attribute::HTTP_ROUTE).as_deref(),
            Some("/hang")
        );
    }

    #[tokio::test]
    async fn records_no_client_address_path_or_query() {
        let (provider, exporter) = meter_provider();
        let trace_layer = TraceLayer::new()
            .with_meter(provider.meter("test"))
            .with_url_scheme("https");

        let client_address: SocketAddr = "192.0.2.7:51234".parse().expect("valid address");
        let mut request = Request::builder()
            .uri("/objects/42?token=secret")
            .header(HOST, "example.com")
            .body(Body::empty())
            .expect("valid request");
        request.extensions_mut().insert(ConnectInfo(client_address));
        router(trace_layer, Attach::RouteLayer)
            .oneshot(request)
            .await
            .expect("router is infallible");

        let points = recorded_attributes(&provider, &exporter);
        assert_eq!(points.len(), 1, "{points:?}");
        let attributes = &points[0];
        assert_eq!(
            value(attributes, semconv::attribute::URL_SCHEME).as_deref(),
            Some("https")
        );
        assert_eq!(value(attributes, semconv::attribute::CLIENT_ADDRESS), None);
        assert!(
            attributes.iter().all(|kv| {
                let value = kv.value.to_string();
                !value.contains("192.0.2.7") && !value.contains("secret") && !value.contains("42")
            }),
            "{attributes:?}"
        );
    }

    #[tokio::test]
    async fn records_the_configured_scheme_for_an_absolute_form_uri() {
        let (provider, exporter) = meter_provider();
        let trace_layer = TraceLayer::new()
            .with_meter(provider.meter("test"))
            .with_url_scheme("https");
        let request = Request::builder()
            .uri("foo://example.com/objects/42")
            .body(Body::empty())
            .expect("valid request");

        router(trace_layer, Attach::RouterLayer)
            .oneshot(request)
            .await
            .expect("router is infallible");

        let points = recorded_attributes(&provider, &exporter);
        assert_eq!(points.len(), 1, "{points:?}");
        assert_eq!(
            value(&points[0], semconv::attribute::URL_SCHEME).as_deref(),
            Some("https")
        );
    }

    #[tokio::test]
    async fn records_the_error_type_of_a_failed_service() {
        let (provider, exporter) = meter_provider();
        let service =
            TraceLayer::new()
                .with_meter(provider.meter("test"))
                .layer(tower::service_fn(|_request: Request| async {
                    Err::<Response, _>(std::io::Error::other("connection reset"))
                }));
        let request = Request::builder()
            .uri("/objects/42")
            .body(Body::empty())
            .expect("valid request");

        service
            .oneshot(request)
            .await
            .expect_err("the inner service fails");

        let points = recorded_attributes(&provider, &exporter);
        assert_eq!(points.len(), 1, "{points:?}");
        let attributes = &points[0];
        assert_eq!(
            value(attributes, semconv::attribute::ERROR_TYPE).as_deref(),
            Some(std::any::type_name::<std::io::Error>())
        );
        assert_eq!(
            value(attributes, semconv::attribute::HTTP_RESPONSE_STATUS_CODE),
            None
        );
    }

    #[tokio::test]
    async fn test() {
        let trace_layer = TraceLayer::new();
        let router = Router::new()
            .route("/", get(|| async { "Hello, World!" }))
            .layer(trace_layer);

        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        axum::serve(listener, router)
            .with_graceful_shutdown(tokio::time::sleep(std::time::Duration::from_secs(1)))
            .await
            .unwrap();
    }
}
