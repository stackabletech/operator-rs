use axum::{extract::State, routing::post, Json, Router};
use kube::core::conversion::ConversionReview;

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
    pub fn new<T>(handler: T, options: Options) -> Self
    where
        T: WebhookHandler<ConversionReview, ConversionReview> + Clone + Send + Sync + 'static,
    {
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
    /// needs to be mutable.
    ///
    /// ### See
    ///
    /// - <https://docs.rs/axum/latest/axum/index.html#sharing-state-with-handlers>
    pub fn new_with_state<T, S>(handler: T, state: S, options: Options) -> Self
    where
        T: StatefulWebhookHandler<ConversionReview, ConversionReview, S>
            + Clone
            + Send
            + Sync
            + 'static,
        S: Clone + Send + Sync + 'static,
    {
        // See https://github.com/async-graphql/async-graphql/discussions/1150
        let handler_fn = |State(state): State<S>, Json(review): Json<ConversionReview>| async {
            let review = handler.call(review, state);
            Json(review)
        };

        let router = Router::new()
            .route("/convert", post(handler_fn))
            .with_state(state);

        Self { router, options }
    }

    pub async fn run(self) -> Result<(), crate::Error> {
        let server = WebhookServer::new(self.router, self.options);
        server.run().await
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use super::*;
    use crate::Options;

    #[derive(Debug, Clone)]
    struct State {
        inner: usize,
    }

    fn handler(req: ConversionReview) -> ConversionReview {
        // In here we can do the CRD conversion
        req
    }

    fn handler_with_state(req: ConversionReview, state: Arc<State>) -> ConversionReview {
        println!("{}", state.inner);
        req
    }

    #[tokio::test]
    async fn without_state() {
        let server = ConversionWebhookServer::new(handler, Options::default());
        server.run().await.unwrap();
    }

    #[tokio::test]
    async fn with_state() {
        let shared_state = Arc::new(State { inner: 0 });
        let server = ConversionWebhookServer::new_with_state(
            handler_with_state,
            shared_state,
            Options::default(),
        );
        server.run().await.unwrap();
    }
}
