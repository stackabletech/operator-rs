use std::{future::Future, net::SocketAddr, task::Poll};

use axum::{
    extract::{ConnectInfo, MatchedPath, Request},
    http::header::USER_AGENT,
    response::Response,
};
use futures_util::ready;
use opentelemetry::trace::SpanKind;
use pin_project::pin_project;
use tower::{Layer, Service};
use tracing::{field::Empty, trace_span, Span};

#[derive(Debug, Default)]
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables various fields marked as opt-in by the specification.
    ///
    /// This will require more computing power and will increase the latency.
    /// See <https://opentelemetry.io/docs/specs/semconv/http/http-spans/>
    pub fn with_opt_in(&mut self) {
        self.opt_in = true
    }
}

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

        let result = ready!(this.future.poll(cx));
        this.span.finalize(&result);

        Poll::Ready(result)
    }
}

pub trait RequestExt {
    fn client_socket_address(&self) -> Option<SocketAddr>;
    fn server_socket_address(&self) -> Option<SocketAddr>;
    fn matched_path(&self) -> Option<&MatchedPath>;
    fn span_name(&self) -> String;
    fn user_agent(&self) -> &str;
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

    fn user_agent(&self) -> &str {
        self.headers()
            .get(USER_AGENT)
            .map_or("", |ua| ua.to_str().unwrap_or_default())
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
    /// Setting the `opt_in` parameter to `true` enables various fields marked
    /// as opt-in by the specification. This will require more computing power
    /// and will increase the latency.
    ///
    /// [1]: https://opentelemetry.io/docs/specs/semconv/http/http-spans/
    fn from_request(req: &Request, opt_in: bool) -> Self;

    /// Finalize the [`Span`] with an Axum [`Response`].
    fn finalize_with_response(&self, response: &Response);

    /// Finalize the [`Span`] with an error.
    fn finalize_with_error<E>(&self, error: E)
    where
        E: std::error::Error;

    /// Finalize the [`Span`] with a result.
    ///
    /// The default implementation internally calls:
    ///
    /// - [`SpanExt::finalize_with_response`] when [`Ok`]
    /// - [`SpanExt::finalize_with_error`] when [`Err`]
    fn finalize<E>(&self, result: &Result<Response, E>)
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
        let user_agent = req.user_agent();
        let span_name = req.span_name();
        let url = req.uri();

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
            otel.kind = ?SpanKind::Server,
            http.request.method = http_method,
            http.response.status_code = Empty,
            url.path = url.path(),
            url.query = url.query(),
            url.scheme = url.scheme_str().unwrap_or_default(),
            // TODO (@Techassi): Add network.protocol.version
            user_agent.original = user_agent,
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

            if opt_in {
                span.record("client.port", client_socket_address.port());
            }
        }

        if opt_in {
            for (header_name, header_value) in req.headers() {
                let header_name = header_name.as_str().to_lowercase();
                let field_name = format!("http.request.header.{header_name}");

                span.record(
                    field_name.as_str(),
                    header_value.to_str().unwrap_or_default(),
                );
            }
        }

        if let Some(http_route) = req.matched_path() {
            span.record("http.route", http_route.as_str());
        }

        span
    }

    fn finalize_with_response(&self, response: &Response) {
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
    }

    fn finalize_with_error<E>(&self, error: E)
    where
        E: std::error::Error,
    {
        self.record("otel.status_code", "Error");

        // NOTE (@Techassi): This field might get renamed: https://github.com/tokio-rs/tracing-opentelemetry/issues/115
        self.record("otel.status_message", error.to_string());
    }
}
