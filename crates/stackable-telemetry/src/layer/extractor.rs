use axum::http::{HeaderMap, HeaderName};
use opentelemetry::{propagation::Extractor, Context};

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
    pub fn new(headers: &'a HeaderMap) -> Self {
        Self(headers)
    }

    pub fn extract_context(&self) -> Context {
        opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}
