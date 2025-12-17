use async_trait::async_trait;
use axum::Router;
pub use conversion_webhook::{ConversionReview, ConversionWebhook, ConversionWebhookError};
use k8s_openapi::{
    ByteString,
    api::admissionregistration::v1::{ServiceReference, WebhookClientConfig},
};
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

    /// Wether the [`Self::handle_certificate_rotation`] function should be called or not
    fn ignore_certificate_rotation(&self) -> bool;

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

/// Returns the client config that can be used in admission webhooks.
///
/// It is used to contact the correct HTTP endpoint, which is determined from the given parameters.
/// (CRD conversions require a similar, but different, client config).
fn get_webhook_client_config(
    options: &WebhookServerOptions,
    ca_bundle: ByteString,
    http_path: impl Into<String>,
) -> WebhookClientConfig {
    WebhookClientConfig {
        service: Some(ServiceReference {
            name: options.webhook_service_name.to_owned(),
            namespace: options.webhook_namespace.to_owned(),
            path: Some(http_path.into()),
            port: Some(options.socket_addr.port().into()),
        }),
        // Here, ByteString takes care of encoding the provided content as base64.
        ca_bundle: Some(ca_bundle),
        url: None,
    }
}
