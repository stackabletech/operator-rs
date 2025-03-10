use axum::http::{HeaderMap, HeaderName, HeaderValue};
use opentelemetry::{propagation::Injector, Context};

/// Injects the [`TextMapPropagator`][1] to propagate trace parent information
/// in HTTP headers.
///
/// This propagation is useful when consumers of the HTTP response want to link
/// up their span data with the data produced in the Tower [`Layer`][2].
/// A concrete usage example is available in the [`TraceService::call`][3]
/// implementation for [`TraceService`][4].
///
/// This is pretty much a copy-pasted version of the [`HeaderInjector`][5] from
/// the `opentelemetry_http` crate. However, we cannot use this crate, as it
/// uses an outdated version of the underlying `http` crate.
///
/// [1]: opentelemetry::propagation::TextMapPropagator
/// [2]: tower::Layer
/// [3]: tower::Service::call
/// [4]: crate::instrumentation::axum::TraceService
/// [5]: https://docs.rs/opentelemetry-http/latest/opentelemetry_http/struct.HeaderInjector.html
pub struct HeaderInjector<'a>(pub(crate) &'a mut HeaderMap);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(header_name) = HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(header_value) = HeaderValue::from_str(&value) {
                self.0.insert(header_name, header_value);
            }
        }
    }
}

impl<'a> HeaderInjector<'a> {
    /// Create a new header injecttor from a mutable reference to [`HeaderMap`].
    pub fn new(headers: &'a mut HeaderMap) -> Self {
        Self(headers)
    }

    /// Inject the [`TextMapPropagator`][1] into the HTTP headers.
    ///
    /// [1]: opentelemetry::propagation::TextMapPropagator
    pub fn inject_context(&mut self, cx: &Context) {
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(cx, self)
        })
    }
}
