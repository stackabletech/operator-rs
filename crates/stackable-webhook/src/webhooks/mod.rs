use async_trait::async_trait;
use axum::Router;
pub use conversion_webhook::{ConversionReview, ConversionWebhook, ConversionWebhookError};
use k8s_openapi::ByteString;
pub use mutating_webhook::{MutatingWebhook, MutatingWebhookError};
use snafu::Snafu;
pub use validating_webhook::{ValidatingWebhook, ValidatingWebhookError};
use x509_cert::Certificate;

use crate::WebhookServerOptions;

mod conversion_webhook;
mod mutating_webhook;
mod validating_webhook;

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

    #[snafu(display("validating webhook error"), context(false))]
    ValidatingWebhookError {
        source: validating_webhook::ValidatingWebhookError,
    },
}

/// A webhook (such as a conversion or mutating webhook) needs to implement this trait.
//
// We still need to use the async-trait crate, as Rust 1.91.1 does not support dynamic dispatch
// in combination with async functions.
#[async_trait]
pub trait Webhook {
    /// The webhook can add arbitrary routes to the passed [`Router`] and needs to return the
    /// resulting [`Router`].
    fn register_routes(&self, router: Router) -> Router;

    /// The HTTPS server periodically rotates it's certificate.
    ///
    /// Typically, some caller of the webhook (e.g. Kubernetes) needs to know the certificate to be
    /// able to establish the TLS connection.
    /// Webhooks are informed about new certificates by this function and can react accordingly.
    async fn handle_certificate_rotation(
        &mut self,
        new_certificate: &Certificate,
        new_ca_bundle: &ByteString,
        options: &WebhookServerOptions,
    ) -> Result<(), WebhookError>;
}
