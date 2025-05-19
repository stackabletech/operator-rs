use std::fmt::Debug;

use axum::{Json, Router, extract::State, routing::post};
// Re-export this type because users of the conversion webhook server require
// this type to write the handler function. Instead of importing this type from
// kube directly, consumers can use this type instead. This also eliminates
// keeping the kube dependency version in sync between here and the operator.
pub use kube::core::conversion::ConversionReview;
use tracing::instrument;

use crate::{StatefulWebhookHandler, WebhookHandler, WebhookServer, options::Options};

impl<F> WebhookHandler<ConversionReview, ConversionReview> for F
where
    F: FnOnce(ConversionReview) -> ConversionReview,
{
    fn call(self, req: ConversionReview) -> ConversionReview {
        self(req)
    }
}

impl<F, S> StatefulWebhookHandler<ConversionReview, ConversionReview, S> for F
where
    F: FnOnce(ConversionReview, S) -> ConversionReview,
{
    fn call(self, req: ConversionReview, state: S) -> ConversionReview {
        self(req, state)
    }
}

/// A ready-to-use CRD conversion webhook server.
///
/// See [`ConversionWebhookServer::new()`] and [`ConversionWebhookServer::new_with_state()`]
/// for usage examples.
pub struct ConversionWebhookServer {
    options: Options,
    router: Router,
}

impl ConversionWebhookServer {
    /// Creates a new conversion webhook server **without** state which expects
    /// POST requests being made to the `/convert/{kind}` endpoints.
    ///
    /// Each request is handled by the provided `handler` functions. Any function
    /// with the signature `(ConversionReview) -> ConversionReview` can be
    /// provided. The [`ConversionReview`] type can be imported via a re-export at
    /// [`crate::servers::ConversionReview`].
    ///
    /// # Example
    ///
    /// ```
    /// use stackable_operator::crd::authentication::core::AuthenticationClass;
    /// use stackable_webhook::{
    ///     servers::{ConversionReview, ConversionWebhookServer},
    ///     Options
    /// };
    ///
    /// let handlers = [(
    ///     "AuthenticationClass",
    ///     AuthenticationClass::convert as fn(ConversionReview) -> ConversionReview,
    /// )];
    ///
    /// // Construct the conversion webhook server
    /// let server = ConversionWebhookServer::new(handlers, Options::default());
    /// ```
    #[instrument(name = "create_conversion_webhook_server", skip(handlers))]
    pub fn new<'a, H>(handlers: impl IntoIterator<Item = (&'a str, H)>, options: Options) -> Self
    where
        H: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
        tracing::debug!("creating new conversion webhook server");

        let mut router = Router::new();
        for (kind, handler) in handlers {
            let handler_fn = |Json(review): Json<ConversionReview>| async {
                let review = handler.call(review);
                Json(review)
            };

            router = router.route(&format!("/convert/{kind}"), post(handler_fn));
        }

        Self { router, options }
    }

    /// Creates a new conversion webhook server **without** state which expects
    /// POST requests being made to the `/convert/{kind}` endpoints.
    ///
    /// Each request is handled by the provided `handler` functions. Any function
    /// with the signature `(ConversionReview, S) -> ConversionReview` can be
    /// provided. The [`ConversionReview`] type can be imported via a re-export at
    /// [`crate::servers::ConversionReview`].
    ///
    /// It is recommended to wrap the state in an [`Arc`][std::sync::Arc] if it
    /// needs to be mutable, see
    /// <https://docs.rs/axum/latest/axum/index.html#sharing-state-with-handlers>.
    ///
    /// # Example
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use stackable_webhook::{
    ///     servers::{ConversionReview, ConversionWebhookServer},
    ///     Options
    /// };
    ///
    /// #[derive(Debug, Clone)]
    /// struct State {}
    ///
    /// let handlers = [(
    ///     "AuthenticationClass",
    ///     auth_class_handler as fn(ConversionReview, state: Arc<State>) -> ConversionReview,
    /// )];
    ///
    /// let shared_state = Arc::new(State {});
    /// let server = ConversionWebhookServer::new_with_state(
    ///     handlers,
    ///     shared_state,
    ///     Options::default(),
    /// );
    ///
    /// // Define the handler function
    /// fn auth_class_handler(req: ConversionReview, state: Arc<State>) -> ConversionReview {
    ///    // In here we can do the CRD conversion
    ///    req
    /// }
    /// ```
    #[instrument(name = "create_conversion_webhook_server_with_state", skip(handlers))]
    pub fn new_with_state<'a, H, S>(
        handlers: impl IntoIterator<Item = (&'a str, H)>,
        state: S,
        options: Options,
    ) -> Self
    where
        H: StatefulWebhookHandler<ConversionReview, ConversionReview, S>
            + Clone
            + Send
            + Sync
            + 'static,
        S: Clone + Debug + Send + Sync + 'static,
    {
        tracing::debug!("creating new conversion webhook server with state");

        let mut router = Router::new();
        for (kind, handler) in handlers {
            // NOTE (@Techassi): Initially, after adding the state extractor, the
            // compiler kept throwing a trait error at me stating that the closure
            // below doesn't implement the Handler trait from Axum. This had nothing
            // to do with the state itself, but rather the order of extractors. All
            // body consuming extractors, like the JSON extractor need to come last
            // in the handler.
            // https://docs.rs/axum/latest/axum/extract/index.html#the-order-of-extractors
            let handler_fn = |State(state): State<S>, Json(review): Json<ConversionReview>| async {
                let review = handler.call(review, state);
                Json(review)
            };

            router = router.route(&format!("/convert/{kind}"), post(handler_fn));
        }
        let router = router.with_state(state);

        Self { router, options }
    }

    /// Starts the conversion webhook server by starting the underlying
    /// [`WebhookServer`].
    pub async fn run(self) -> Result<(), crate::Error> {
        tracing::info!("starting conversion webhook server");

        let server = WebhookServer::new(self.router, self.options);
        server.run().await
    }
}
