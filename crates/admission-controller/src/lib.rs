use axum::{routing::post, Router};

pub mod webhooks;

pub struct AdmissionController {
    router: Router,
}

impl AdmissionController {
    pub fn new() -> Self {
        let router = Router::new().route("/", post(|| async {}));

        Self { router }
    }

    pub async fn run(self) {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        axum::serve(listener, self.router).await.unwrap();
    }
}
