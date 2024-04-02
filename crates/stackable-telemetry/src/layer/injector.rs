use axum::http::{HeaderMap, HeaderName, HeaderValue};
use opentelemetry::{propagation::Injector, Context};

pub struct HeaderInjector<'a>(pub(crate) &'a mut HeaderMap);

impl<'a> Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(header_name) = HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(header_value) = HeaderValue::from_str(&value) {
                self.0.insert(header_name, header_value);
            }
        }
    }
}

impl<'a> HeaderInjector<'a> {
    pub fn new(headers: &'a mut HeaderMap) -> Self {
        Self(headers)
    }

    pub fn inject_context(&mut self, cx: &Context) {
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(cx, self)
        })
    }
}
