//! Conversion of HTTP requests and responses into the attribute values defined by the
//! [HTTP semantic conventions][1].
//!
//! [`TraceLayer`](super::TraceLayer) records its spans and its `http.server.request.duration`
//! metric from the same [`ServerRequestAttributes`], so both have the same method, scheme, route
//! and protocol version for a request.
//!
//! [1]: https://opentelemetry.io/docs/specs/semconv/http/
use axum::{
    extract::{MatchedPath, Request},
    http::{Method, StatusCode, Version},
};
use opentelemetry::KeyValue;
use opentelemetry_semantic_conventions::attribute::{
    ERROR_TYPE, HTTP_REQUEST_METHOD, HTTP_RESPONSE_STATUS_CODE, HTTP_ROUTE,
    NETWORK_PROTOCOL_VERSION, URL_SCHEME,
};

/// The value recorded for an HTTP method the conventions do not list as known.
pub const OTHER_METHOD: &str = "_OTHER";

/// The method part of a span name for a request whose method is [`OTHER_METHOD`].
const OTHER_METHOD_SPAN_NAME: &str = "HTTP";

/// Returns the `http.request.method` value: the method name for the methods the conventions list
/// as known, [`OTHER_METHOD`] for any other, so that a client cannot create unbounded attribute
/// values.
pub fn http_request_method(method: &Method) -> &'static str {
    match *method {
        Method::CONNECT => "CONNECT",
        Method::DELETE => "DELETE",
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        Method::PATCH => "PATCH",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::TRACE => "TRACE",
        _ if method.as_str() == "QUERY" => "QUERY",
        _ => OTHER_METHOD,
    }
}

/// Returns the `network.protocol.version` value, or [`None`] for a version the conventions name
/// no value for.
pub fn network_protocol_version(version: Version) -> Option<&'static str> {
    match version {
        Version::HTTP_09 => Some("0.9"),
        Version::HTTP_10 => Some("1.0"),
        Version::HTTP_11 => Some("1.1"),
        Version::HTTP_2 => Some("2"),
        Version::HTTP_3 => Some("3"),
        _ => None,
    }
}

/// Returns the `http.route` value: the route template axum matched, never the raw path.
///
/// Axum inserts [`MatchedPath`] while routing, so it is only present for a layer added through
/// `Router::layer` or `Router::route_layer`, and only for requests a route matched. A layer that
/// wraps the whole `Router` as a service, and the router's fallback, see no route.
pub fn http_route(request: &Request) -> Option<&str> {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
}

/// Returns the span name the conventions define for a server request: `{method} {http.route}`, or
/// `{method}` without a route, where `method` is the value of [`http_request_method`] and
/// [`OTHER_METHOD`] is written as `HTTP`.
pub fn span_name(method: &str, route: Option<&str>) -> String {
    let method = if method == OTHER_METHOD {
        OTHER_METHOD_SPAN_NAME
    } else {
        method
    };

    match route {
        Some(route) => format!("{method} {route}"),
        None => method.to_owned(),
    }
}

/// How a request ended, which decides the status and `error.type` attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestOutcome {
    /// The server produced a response with this status.
    Response(StatusCode),

    /// The inner service failed before a response existed. The value is the `error.type`, by
    /// convention the type name of the error.
    ///
    /// [`std::any::type_name`] does not guarantee a stable output, so a compiler update can change
    /// these values.
    Failed(&'static str),
}

/// The attributes of a request that both the per-request span of [`TraceLayer`](super::TraceLayer)
/// and its `http.server.request.duration` metric have.
///
/// Each field is the value of the HTTP semantic convention attribute its doc names.
///
/// It contains only attributes the conventions list for the metric, which excludes client
/// addresses, headers and the raw URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerRequestAttributes {
    /// `http.request.method`, see [`http_request_method`].
    pub method: &'static str,

    /// `url.scheme`.
    pub scheme: &'static str,

    /// `http.route`, see [`http_route`].
    pub route: Option<String>,

    /// `network.protocol.version`, see [`network_protocol_version`].
    pub protocol_version: Option<&'static str>,
}

impl ServerRequestAttributes {
    /// Extracts the attributes from `request`, with `scheme` as the scheme the server listens with.
    ///
    /// The scheme in the request URI is ignored: the client controls it, through the HTTP/2
    /// `:scheme` pseudo-header or an HTTP/1 request in absolute form, and a value taken from it
    /// would let any client create new metric time series.
    pub fn from_request(request: &Request, scheme: &'static str) -> Self {
        Self {
            method: http_request_method(request.method()),
            scheme,
            route: http_route(request).map(str::to_owned),
            protocol_version: network_protocol_version(request.version()),
        }
    }

    /// Returns the metric attributes, completed with the status or error of `outcome`.
    pub fn into_key_values(self, outcome: RequestOutcome) -> Vec<KeyValue> {
        let mut attributes = vec![
            KeyValue::new(HTTP_REQUEST_METHOD, self.method),
            KeyValue::new(URL_SCHEME, self.scheme),
        ];
        if let Some(route) = self.route {
            attributes.push(KeyValue::new(HTTP_ROUTE, route));
        }
        if let Some(protocol_version) = self.protocol_version {
            attributes.push(KeyValue::new(NETWORK_PROTOCOL_VERSION, protocol_version));
        }
        match outcome {
            RequestOutcome::Response(status) => {
                attributes.push(KeyValue::new(
                    HTTP_RESPONSE_STATUS_CODE,
                    i64::from(status.as_u16()),
                ));
                // The conventions only treat server errors as failures, not client errors.
                if status.is_server_error() {
                    attributes.push(KeyValue::new(ERROR_TYPE, status.as_u16().to_string()));
                }
            }
            RequestOutcome::Failed(error_type) => {
                attributes.push(KeyValue::new(ERROR_TYPE, error_type));
            }
        }
        attributes
    }
}

#[cfg(test)]
mod test {
    use axum::body::Body;
    use rstest::rstest;

    use super::*;

    #[test]
    fn known_methods_keep_their_name() {
        assert_eq!(http_request_method(&Method::GET), "GET");
        assert_eq!(
            http_request_method(&Method::from_bytes(b"QUERY").expect("valid method")),
            "QUERY"
        );
    }

    #[test]
    fn unknown_methods_are_other() {
        let method = Method::from_bytes(b"BREW").expect("valid method");
        assert_eq!(http_request_method(&method), OTHER_METHOD);
    }

    #[rstest]
    #[case("GET", Some("/objects/{id}"), "GET /objects/{id}")]
    #[case("POST", None, "POST")]
    #[case(OTHER_METHOD, Some("/objects/{id}"), "HTTP /objects/{id}")]
    #[case(OTHER_METHOD, None, "HTTP")]
    fn span_name_follows_the_conventions(
        #[case] method: &str,
        #[case] route: Option<&str>,
        #[case] expected: &str,
    ) {
        assert_eq!(span_name(method, route), expected);
    }

    #[test]
    fn attributes_from_origin_form_request() {
        let request = Request::builder()
            .uri("/objects/42?token=secret")
            .version(Version::HTTP_11)
            .body(Body::empty())
            .expect("valid request");

        let attributes = ServerRequestAttributes::from_request(&request, "https");

        assert_eq!(
            attributes,
            ServerRequestAttributes {
                method: "GET",
                scheme: "https",
                route: None,
                protocol_version: Some("1.1"),
            }
        );
    }

    #[test]
    fn absolute_form_request_records_the_configured_scheme() {
        let request = Request::builder()
            .uri("foo://example.com/objects")
            .body(Body::empty())
            .expect("valid request");

        let attributes = ServerRequestAttributes::from_request(&request, "https");

        assert_eq!(attributes.scheme, "https");
    }

    #[rstest]
    #[case(RequestOutcome::Response(StatusCode::OK), Some("200"), None)]
    #[case(RequestOutcome::Response(StatusCode::NOT_FOUND), Some("404"), None)]
    #[case(
        RequestOutcome::Response(StatusCode::SERVICE_UNAVAILABLE),
        Some("503"),
        Some("503")
    )]
    #[case(
        RequestOutcome::Failed("std::io::error::Error"),
        None,
        Some("std::io::error::Error")
    )]
    fn key_values_follow_the_outcome(
        #[case] outcome: RequestOutcome,
        #[case] expected_status: Option<&str>,
        #[case] expected_error_type: Option<&str>,
    ) {
        let attributes = ServerRequestAttributes {
            method: "POST",
            scheme: "http",
            route: Some("/objects/{id}".to_owned()),
            protocol_version: Some("2"),
        };

        let key_values = attributes.into_key_values(outcome);
        let value = |key: &str| {
            key_values
                .iter()
                .find(|kv| kv.key.as_str() == key)
                .map(|kv| kv.value.to_string())
        };

        assert_eq!(value(HTTP_RESPONSE_STATUS_CODE).as_deref(), expected_status);
        assert_eq!(value(ERROR_TYPE).as_deref(), expected_error_type);
        assert_eq!(value(HTTP_ROUTE).as_deref(), Some("/objects/{id}"));
        assert_eq!(value(NETWORK_PROTOCOL_VERSION).as_deref(), Some("2"));
    }
}
