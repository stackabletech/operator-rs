// use std::{net::SocketAddr, ops::Deref};

// use axum::{
//     routing::{post, MethodRouter},
//     Json,
// };
// use kube::core::conversion::{ConversionRequest, ConversionResponse};

// use crate::{Handlers, WebhookServer};

// pub struct ConversionWebhookServer(WebhookServer<ConversionHandlers>);

// impl Deref for ConversionWebhookServer {
//     type Target = WebhookServer<ConversionHandlers>;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl ConversionWebhookServer {
//     pub async fn new(socket_addr: SocketAddr) -> Self {
//         Self(WebhookServer::new(socket_addr, ConversionHandlers).await)
//     }
// }

// pub struct ConversionHandlers;

// impl Handlers for ConversionHandlers {
//     fn endpoints<T>(&self) -> Vec<(&str, MethodRouter<T>)>
//     where
//         T: Clone + Sync + Send + 'static,
//     {
//         vec![("/convert", post(convert_handler))]
//     }
// }

// async fn convert_handler(
//     Json(_conversion_request): Json<ConversionRequest>,
// ) -> Json<ConversionResponse> {
//     todo!()
// }
