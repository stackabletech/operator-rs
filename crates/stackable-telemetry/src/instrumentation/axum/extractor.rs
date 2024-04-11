use axum::http::{HeaderMap, HeaderName};
use opentelemetry::{propagation::Extractor, Context};

/// Extracts the [`TextMapPropagator`][1] to access trace parent information in
/// HTTP headers.
///
/// This propagation is useful when an HTTP request already has a trace parent
/// which can be picked up by the Tower [`Layer`][2] to link both spans together.
/// A concrete usage example is available in [`SpanExt::from_request`][3].
///
/// [1]: opentelemetry::propagation::TextMapPropagator
/// [2]: tower::Layer
pub struct HeaderExtractor<'a>(pub(crate) &'a HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .get(key)
            .and_then(|header_value| header_value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(HeaderName::as_str).collect()
    }
}

impl<'a> HeaderExtractor<'a> {
    /// Create a new header extractor from a reference to a [`HeaderMap`].
    pub fn new(headers: &'a HeaderMap) -> Self {
        Self(headers)
    }

    /// Extracts the [`TextMapPropagator`][1] from the HTTP headers.
    ///
    /// [1]: opentelemetry::propagation::TextMapPropagator
    pub fn extract_context(&self) -> Context {
        opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}
