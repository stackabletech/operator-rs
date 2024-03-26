use std::{future::Future, net::SocketAddr};

use axum::{
    extract::{ConnectInfo, MatchedPath, Request},
    response::Response,
};
use tower::{Layer, Service};
use tracing::{field::Empty, trace_span, Span};

#[derive(Debug)]
pub struct TraceLayer;

impl<S> Layer<S> for TraceLayer {
    type Service = TraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceService { inner }
    }
}

#[derive(Debug, Clone)]
pub struct TraceService<S> {
    inner: S,
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
        let span = Span::from_request(&req);

        let future = {
            let _guard = span.enter();
            self.inner.call(req)
        };

        ResponseFuture { future, span }
    }
}

pub struct ResponseFuture<F> {
    pub(crate) future: F,
    pub(crate) span: Span,
}

impl<F, O, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<O>, E>>,
    E: std::error::Error + 'static,
{
    type Output = Result<Response<O>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}

pub trait RequestExt {
    fn client_socket_address(&self) -> Option<SocketAddr>;
    fn server_socket_address(&self) -> Option<SocketAddr>;
    fn matched_path(&self) -> Option<&MatchedPath>;
    fn span_name(&self) -> String;
}

impl RequestExt for Request {
    fn server_socket_address(&self) -> Option<SocketAddr> {
        todo!()
    }

    fn client_socket_address(&self) -> Option<SocketAddr> {
        self.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0)
    }

    fn span_name(&self) -> String {
        let http_method = self.method().as_str();

        match self.matched_path() {
            Some(matched_path) => format!("{http_method} {}", matched_path.as_str()),
            None => http_method.to_string(),
        }
    }

    fn matched_path(&self) -> Option<&MatchedPath> {
        self.extensions().get::<MatchedPath>()
    }
}

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
    /// [1]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/
    fn from_request(req: &Request) -> Self;
}

impl SpanExt for Span {
    fn from_request(req: &Request) -> Self {
        let http_method = req.method().as_str();
        let span_name = req.span_name();

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

        // Setting common fields first
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#common-attributes

        let span = trace_span!(
            "HTTP request",
            otel.name = span_name,
            http.request.method = http_method,
            http.response.status_code = Empty,
        );

        // Setting server.address and server.port
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#setting-serveraddress-and-serverport-attributes

        if let Some(server_socket_address) = req.server_socket_address() {
            span.record("server.address", server_socket_address.ip().to_string());
            span.record("server.port", server_socket_address.port());
        }

        // Setting fields according to the HTTP server semantic conventions
        // See https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-server-semantic-conventions

        if let Some(client_socket_address) = req.client_socket_address() {
            span.record("client.address", client_socket_address.ip().to_string());
            span.record("client.port", client_socket_address.port());
        }

        let headers = req.headers();

        if let Some(http_route) = req.matched_path() {
            span.record("http.route", http_route.as_str());
        }

        span
    }
}
