use std::net::SocketAddr;

use axum::{routing::MethodRouter, Router};
use tokio::net::TcpListener;

pub mod conversion;

pub trait Handlers {
    fn endpoints<T>(&self) -> Vec<(&str, MethodRouter<T>)>
    where
        T: Clone + Sync + Send + 'static;
}

pub struct WebhookServer<T>
where
    T: Handlers,
{
    socket_addr: SocketAddr,
    handlers: T,
}

impl<T> WebhookServer<T>
where
    T: Handlers,
{
    pub async fn new(socket_addr: SocketAddr, handlers: T) -> Self {
        Self {
            socket_addr,
            handlers,
        }
    }

    pub async fn run(&self) {
        let mut router = Router::new();

        for (path, method_router) in self.handlers.endpoints() {
            router = router.route(path, method_router)
        }

        let listener = TcpListener::bind(self.socket_addr).await.unwrap();
        axum::serve(listener, router).await.unwrap()
    }
}
