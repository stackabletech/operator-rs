//! Facilities for reporting Kubernetes controller outcomes
//!
//! The primary entry point is [`report_controller_reconciled`].

use std::error::Error;

use kube::{
    core::DynamicObject,
    runtime::{
        controller::{self, Action},
        reflector::ObjectRef,
    },
    Resource,
};
use tracing;

use crate::{client::Client, logging::k8s_events::publish_controller_error_as_k8s_event};

/// [`Error`] extensions that help report reconciliation errors
///
/// This should be implemented for reconciler error types.
pub trait ReconcilerError: Error {
    /// `PascalCase`d name for the error category
    ///
    /// This can typically be implemented by delegating to [`strum::EnumDiscriminants`] and [`strum::IntoStaticStr`].
    fn category(&self) -> &'static str;

    /// A reference to a secondary object providing additional context, if any
    ///
    /// This should be [`Some`] if the error happens while evaluating some related object
    /// (for example: when writing a [`StatefulSet`] owned by your controller object).
    ///
    /// [`StatefulSet`]: `k8s_openapi::api::apps::v1::StatefulSet`
    fn secondary_object(&self) -> Option<ObjectRef<DynamicObject>> {
        None
    }
}

/// Reports the controller reconciliation result to all relevant targets
///
/// Currently this means that the result is reported to:
/// * The current [`tracing::Subscriber`], typically at least stderr
/// * Kubernetes [`Event`]s, if there is an error that is relevant to the end user
///
/// [`Event`]: `k8s_openapi::api::events::v1::Event`
pub fn report_controller_reconciled<K, ReconcileErr, QueueErr>(
    client: &Client,
    controller_name: &str,
    result: &Result<(ObjectRef<K>, Action), controller::Error<ReconcileErr, QueueErr>>,
) where
    K: Resource,
    ReconcileErr: ReconcilerError,
    QueueErr: std::error::Error,
{
    match result {
        Ok((obj, _)) => {
            tracing::info!(
                controller.name = controller_name,
                object = %obj,
                "Reconciled object"
            );
        }
        Err(err) => report_controller_error(client, controller_name, err),
    }
}

/// Reports an error to the operator administrator and, if relevant, the end user
fn report_controller_error<ReconcileErr, QueueErr>(
    client: &Client,
    controller_name: &str,
    error: &controller::Error<ReconcileErr, QueueErr>,
) where
    ReconcileErr: ReconcilerError,
    QueueErr: std::error::Error,
{
    tracing::error!(
        controller.name = controller_name,
        error = &*error as &dyn std::error::Error,
        "Failed to reconcile object",
    );
    publish_controller_error_as_k8s_event(client, controller_name, error);
}
