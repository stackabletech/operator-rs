use axum::{routing::post, Json, Router};
use kube::core::conversion::ConversionReview;

use crate::{options::Options, WebhookHandler, WebhookServer};

impl<F> WebhookHandler<ConversionReview, ConversionReview> for F
where
    F: FnOnce(ConversionReview) -> ConversionReview,
{
    fn call(self, req: ConversionReview) -> ConversionReview {
        self(req)
    }
}

pub struct ConversionWebhookServer {
    options: Options,
    router: Router,
}

impl ConversionWebhookServer {
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

    pub async fn run(self) -> Result<(), crate::Error> {
        let server = WebhookServer::new(self.router, self.options);
        server.run().await
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Options;

    fn handler(req: ConversionReview) -> ConversionReview {
        // In here we can do the CRD conversion
        req
    }

    #[tokio::test]
    async fn test() {
        let server = ConversionWebhookServer::new(handler, Options::default());
        server.run().await.unwrap();
    }
}
