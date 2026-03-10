//! Facilities for reporting Kubernetes controller outcomes
//!
//! The primary entry point is [`report_controller_reconciled`].

use std::error::Error;

use kube::{
    Resource,
    core::DynamicObject,
    runtime::{
        controller::{self, Action},
        events::Recorder,
        reflector::ObjectRef,
    },
};
use tracing;

use crate::logging::k8s_events::publish_controller_error_as_k8s_event;

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
pub async fn report_controller_reconciled<K, ReconcileErr, QueueErr>(
    recorder: &Recorder,
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
        Err(controller_error) => {
            match controller_error {
                // Errors raised from queued stuff we will mark as _warning_.
                // We can't easily discriminate any further.
                controller::Error::QueueError(queue_error) => tracing::warn!(
                    controller.name = controller_name,
                    error = queue_error as &dyn std::error::Error,
                    "Queued reconcile resulted in an error"
                ),
                // Assume others are _error_ level.
                // NOTE (@NickLarsenNZ): Keeping the same error message as before,
                // but am not sure if it is correct
                _ => tracing::error!(
                    controller.name = controller_name,
                    error = controller_error as &dyn std::error::Error,
                    "Failed to reconcile object"
                ),
            };

            publish_controller_error_as_k8s_event(recorder, controller_error).await;
        }
    }
}
