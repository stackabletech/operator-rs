use async_trait::async_trait;
use axum::Router;
pub use conversion_webhook::{ConversionReview, ConversionWebhook, ConversionWebhookError};
use k8s_openapi::ByteString;
pub use mutating_webhook::{MutatingWebhook, MutatingWebhookError};
use snafu::Snafu;
use x509_cert::Certificate;

use crate::WebhookServerOptions;

mod conversion_webhook;
mod mutating_webhook;

#[derive(Snafu, Debug)]
pub enum WebhookError {
    #[snafu(display("conversion webhook error"), context(false))]
    ConversionWebhookError {
        source: conversion_webhook::ConversionWebhookError,
    },

    #[snafu(display("mutating webhook error"), context(false))]
    MutatingWebhookError {
        source: mutating_webhook::MutatingWebhookError,
    },
}

// We still need to use the async-trait crate, as Rust 1.91.1 does not support dynamic dispatch
// in combination with async functions.
#[async_trait]
pub trait Webhook {
    fn register_routes(&self, router: Router) -> Router;

    async fn handle_certificate_rotation(
        &mut self,
        new_certificate: &Certificate,
        new_ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError>;
}
