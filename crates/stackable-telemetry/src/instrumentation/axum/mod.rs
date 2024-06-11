//! This module contains types which can be used as [`axum`] layers to produce
//! [OpenTelemetry][1] compatible [HTTP spans][2].
//!
//! These spans include a wide variety of fields / attributes defined by the
//! semantic conventions specification. A few examples are:
//!
//! - `http.request.method`
//! - `http.response.status_code`
//! - `user_agent.original`
//!
//! [1]: https://opentelemetry.io/
//! [2]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/
use std::{future::Future, net::SocketAddr, task::Poll};

use axum::{
    extract::{ConnectInfo, MatchedPath, Request},
    http::{
        header::{HOST, USER_AGENT},
        HeaderMap,
    },
    response::Response,
};
use futures_util::ready;
use opentelemetry::trace::SpanKind;
use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::{debug, field::Empty, instrument, trace_span, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod extractor;
mod injector;

pub use extractor::*;
pub use injector::*;

const X_FORWARDED_HOST_HEADER_KEY: &str = "X-Forwarded-Host";

/// A Tower [`Layer`][1] which decorates [`TraceService`].
///
/// ### Example with Axum
///
/// ```
/// use stackable_telemetry::AxumTraceLayer;
/// use axum::{routing::get, Router};
///
/// let trace_layer = AxumTraceLayer::new();
/// let router = Router::new()
///     .route("/", get(|| async { "Hello, World!" }))
///     .layer(trace_layer);
///
/// # let _: Router = router;
/// ```
///
/// ### Example with Webhook
///
/// The usage is even simpler when combined with the `stackable_webhook` crate.
/// The webhook server has built-in support to automatically emit HTTP spans on
/// every incoming request.
///
/// ```
/// use stackable_webhook::{WebhookServer, Options};
/// use axum::Router;
///
/// let router = Router::new();
/// let server = WebhookServer::new(router, Options::default());
///
/// # let _: WebhookServer = server;
/// ```
///
/// This layer is implemented based on [this][1] official Tower guide.
///
/// [1]: https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md
#[derive(Clone, Debug, Default)]
pub struct TraceLayer {
    opt_in: bool,
}

impl<S> Layer<S> for TraceLayer {
    type Service = TraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceService {
            inner,
            opt_in: self.opt_in,
        }
    }
}

impl TraceLayer {
    /// Creates a new default trace layer.
    #[instrument(name = "create_trace_layer")]
    pub fn new() -> Self {
        Self::default()
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

/// A Tower [`Service`] which injects Span Context into HTTP Response Headers.
#[derive(Debug, Clone)]
pub struct TraceService<S> {
    inner: S,
    opt_in: bool,
}

impl<S> Service<Request> for TraceService<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Error: std::error::Error + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let span = Span::from_request(&req, self.opt_in);

        let future = {
            let _guard = span.enter();
            self.inner.call(req)
        };

        ResponseFuture { future, span }
    }
}

/// This future contains the inner service future and the current [`Span`].
#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    pub(crate) future: F,
    pub(crate) span: Span,
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

        Poll::Ready(result)
    }
}

/// This trait provides various helper functions to extract data from a
/// HTTP [`Request`].
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
    ///   server.address and server.port attributes and SHOULD determine them by
    ///   using the first of the following that applies:
    /// >
    /// > - The original host which may be passed by the reverse proxy in the
    /// >  Forwarded#host, X-Forwarded-Host, or a similar header.
    /// > - The :authority pseudo-header in case of HTTP/2 or HTTP/3
    /// > - The Host header.
    ///
    /// [1]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes
    fn server_host(&self) -> Option<String>;

    /// Returns the matched path, like `/object/:object_id/tags`.
    ///
    /// The returned path has low cardinality. It will never contain any path
    /// or query parameter. This behaviour is suggested by the conventions
    /// specification.
    fn matched_path(&self) -> Option<&MatchedPath>;

    /// Returns the span name.
    ///
    /// The format is either `{method} {http.route}` or `{method}` if
    /// `http.route` is not available. Examples are:
    ///
    /// - `GET /object/:object_id/tags`
    /// - `PUT /upload/:file_id`
    /// - `POST /convert`
    /// - `OPTIONS`
    fn span_name(&self) -> String;

    /// Returns the user agent, if available.
    fn user_agent(&self) -> Option<&str>;
}

impl RequestExt for Request {
    fn server_host(&self) -> Option<String> {
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
            return Some(host.to_owned());
        }

        if let Some(host) = self.headers().get(HOST).and_then(|host| host.to_str().ok()) {
            return Some(host.to_owned());
        }

        if let Some(host) = self.uri().host() {
            return Some(host.to_owned());
        }

        None
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
        let http_method = self.method().as_str();

        match self.matched_path() {
            Some(matched_path) => format!("{http_method} {}", matched_path.as_str()),
            None => http_method.to_string(),
        }
    }

    fn user_agent(&self) -> Option<&str> {
        self.headers()
            .get(USER_AGENT)
            .map(|ua| ua.to_str().unwrap_or_default())
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
    /// Setting the `opt_in` parameter to `true` enables various fields marked
    /// as opt-in by the specification. This will require more computing power
    /// and will increase the latency.
    ///
    /// [1]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/
    fn from_request(req: &Request, opt_in: bool) -> Self;

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
    fn from_request(req: &Request, opt_in: bool) -> Self {
        let http_method = req.method().as_str();
        let span_name = req.span_name();
        let url = req.uri();

        debug!(
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
        // upstream PR to add support for it, but it was unexpectingly closed.
        // See https://github.com/tokio-rs/tracing/pull/2254.
        //
        // If this is eventually supported (maybe with our efforts), we can use
        // the opentelemetry-semantic-conventions crate, see here:
        // https://docs.rs/opentelemetry-semantic-conventions/latest/opentelemetry_semantic_conventions/index.html

        // Setting common fields first
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#common-attributes

        debug!("create http span");
        let span = trace_span!(
            "HTTP request",
            otel.name = span_name,
            otel.kind = ?SpanKind::Server,
            otel.status_code = Empty,
            otel.status_message = Empty,
            http.request.method = http_method,
            http.response.status_code = Empty,
            url.path = url.path(),
            url.query = url.query(),
            url.scheme = url.scheme_str().unwrap_or_default(),
            user_agent.original = Empty,
            server.address = Empty,
            server.port = Empty,
            client.address = Empty,
            client.port = Empty,
            http.route = Empty,
            http.response.status_code = Empty,
            // TODO (@Techassi): Add network.protocol.version
        );

        // Set the parent span based on the extracted context
        debug!("set parent span based on extracted context");
        let parent = HeaderExtractor::new(req.headers()).extract_context();
        span.set_parent(parent);

        if let Some(user_agent) = req.user_agent() {
            span.record("user_agent.original", user_agent);
        }

        // Setting server.address and server.port
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes

        if let Some(host) = req.server_host() {
            // TODO (@Techassi): Get a little more clever about parsing the host
            // info as IP address and port
            span.record("server.address", host);
            // .record("server.port", server_addr.port());
        }

        // Setting fields according to the HTTP server semantic conventions
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-server-semantic-conventions

        if let Some(client_socket_address) = req.client_socket_address() {
            span.record("client.address", client_socket_address.ip().to_string());

            if opt_in {
                span.record("client.port", client_socket_address.port());
            }
        }

        // Only include the headers if the user opted in, because this might
        // potentially be an expensive operation when many different headers
        // are present. The OpenTelemetry spec also marks this as opt-in.

        // NOTE (@Techassi): Currently, tracing doesn't support recording
        // fields which are not registered at span creation which thus makes
        // it impossible to record request headers at runtime.
        // See: https://github.com/tokio-rs/tracing/issues/1343

        // FIXME (@Techassi): Add support for this when tracing allows dynamic
        // fields.
        // if opt_in {
        //     span.add_header_fields(req.headers())
        // }

        if let Some(http_route) = req.matched_path() {
            span.record("http.route", http_route.as_str());
        }

        span
    }

    fn add_header_fields(&self, headers: &HeaderMap) {
        for (header_name, header_value) in headers {
            // TODO (@Techassi): Add an allow list for header names
            // TODO (@Techassi): Handle multiple headers with the same name

            // header_name.as_str() always returns lowercase strings and thus we
            // don't need to call to_lowercase on it.
            let header_name = header_name.as_str();
            let field_name = format!("http.request.header.{header_name}");

            self.record(
                field_name.as_str(),
                header_value.to_str().unwrap_or_default(),
            );
        }
    }

    fn finalize_with_response(&self, response: &mut Response) {
        let status_code = response.status();
        self.record("http.response.status_code", status_code.as_u16());

        // Only set the span status to "Error" when we encountered an server
        // error. See:
        //
        // - https://opentelemetry.io/docs/specs/semconv/http/http-spans/#status
        // - https://github.com/open-telemetry/opentelemetry-specification/blob/v1.26.0/specification/trace/api.md#set-status
        if status_code.is_server_error() {
            self.record("otel.status_code", "Error");
            // NOTE (@Techassi): Can we add a status_description here as well?
        }

        let mut injector = HeaderInjector::new(response.headers_mut());
        injector.inject_context(&Span::current().context());
    }

    fn finalize_with_error<E>(&self, error: &mut E)
    where
        E: std::error::Error,
    {
        // NOTE (@Techassi): This field might get renamed: https://github.com/tokio-rs/tracing-opentelemetry/issues/115
        self.record("otel.status_code", "Error")
            .record("otel.status_message", error.to_string());
    }
}

#[cfg(test)]
mod test {
    use axum::{routing::get, Router};

    use super::*;

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
