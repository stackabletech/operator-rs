use std::fmt::Debug;

use axum::{extract::State, routing::post, Json, Router};
use kube::core::conversion::ConversionReview;
use tracing::{debug, instrument};

use crate::{options::Options, StatefulWebhookHandler, WebhookHandler, WebhookServer};

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
    /// POST requests being made to the `/convert` endpoint.
    ///
    /// Each request is handled by the provided `handler` function. Any function
    /// with the signature `(ConversionReview) -> ConversionReview` can be
    /// provided.
    ///
    /// # Example
    ///
    /// ```
    /// use stackable_webhook::{servers::ConversionWebhookServer, Options};
    /// use kube::core::conversion::ConversionReview;
    ///
    /// // Construct the conversion webhook server
    /// let server = ConversionWebhookServer::new(handler, Options::default());
    ///
    /// // Define the handler function
    /// fn handler(req: ConversionReview) -> ConversionReview {
    ///    // In here we can do the CRD conversion
    ///    req
    /// }
    /// ```
    #[instrument(name = "create_conversion_webhhok_server", skip(handler))]
    pub fn new<H>(handler: H, options: Options) -> Self
    where
        H: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
        debug!("create new conversion webhook server");

        let handler_fn = |Json(review): Json<ConversionReview>| async {
            let review = handler.call(review);
            Json(review)
        };

        let router = Router::new().route("/convert", post(handler_fn));
        Self { router, options }
    }

    /// Creates a new conversion webhook server **with** state which expects
    /// POST requests being made to the `/convert` endpoint.
    ///
    /// Each request is handled by the provided `handler` function. Any function
    /// with the signature `(ConversionReview, S) -> ConversionReview` can be
    /// provided.
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
    /// use stackable_webhook::{servers::ConversionWebhookServer, Options};
    /// use kube::core::conversion::ConversionReview;
    ///
    /// #[derive(Debug, Clone)]
    /// struct State {}
    ///
    /// let shared_state = Arc::new(State {});
    /// let server = ConversionWebhookServer::new_with_state(
    ///     handler,
    ///     shared_state,
    ///     Options::default(),
    /// );
    ///
    /// // Define the handler function
    /// fn handler(req: ConversionReview, state: Arc<State>) -> ConversionReview {
    ///    // In here we can do the CRD conversion
    ///    req
    /// }
    /// ```
    #[instrument(name = "create_conversion_webhook_server_with_state", skip(handler))]
    pub fn new_with_state<H, S>(handler: H, state: S, options: Options) -> Self
    where
        H: StatefulWebhookHandler<ConversionReview, ConversionReview, S>
            + Clone
            + Send
            + Sync
            + 'static,
        S: Clone + Debug + Send + Sync + 'static,
    {
        debug!("create new conversion webhook server with state");

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

        debug!("create router");
        let router = Router::new()
            .route("/convert", post(handler_fn))
            .with_state(state);

        Self { router, options }
    }

    /// Starts the conversion webhook server by starting the underlying
    /// [`WebhookServer`].
    #[instrument(name = "run_conversion_webhook_server", skip(self), fields(self.options))]
    pub async fn run(self) -> Result<(), crate::Error> {
        debug!("run conversion webhook server");

        let server = WebhookServer::new(self.router, self.options);
        server.run().await
    }
}
