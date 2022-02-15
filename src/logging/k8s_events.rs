//! Utilities for publishing Kubernetes events

use std::error::Error;

use crate::client::Client;
use kube::{
    core::DynamicObject,
    runtime::{
        controller,
        events::{Event, EventType, Recorder, Reporter},
        reflector::ObjectRef,
    },
};
use tracing::Instrument;

/// [`Error`] extensions that help serialize it to helpful [`Event`]s
///
/// This should be implemented for reconciler error types.
pub trait PublishableError: Error {
    /// `PascalCase`d name for the error category
    ///
    /// This can typically be implemented by delegating to [`strum_macros::EnumDiscriminants`] and [`strum_macros::IntoStaticStr`].
    fn variant_name(&self) -> &'static str;

    /// A reference to a secondary object providing additional context, if any
    ///
    /// This should be [`Some`] if the error happens while evaluating some related object
    /// (for example: when writing a [`StatefulSet`] owned by your controller object).
    fn secondary_object(&self) -> Option<ObjectRef<DynamicObject>> {
        None
    }
}

/// Converts an [`Error`] into a publishable Kubernetes [`Event`]
fn error_to_event<E: PublishableError>(err: &E) -> Event {
    // Walk the whole error chain, so that we get all the full reason for the error
    let full_msg = {
        use std::fmt::Write;
        let mut buf = err.to_string();
        let mut err: &dyn Error = err;
        loop {
            err = match err.source() {
                Some(err) => {
                    write!(buf, ": {}", err).unwrap();
                    err
                }
                None => break buf,
            }
        }
    };
    Event {
        type_: EventType::Warning,
        reason: err.variant_name().to_string(),
        note: Some(full_msg),
        action: "Reconcile".to_string(),
        secondary: err.secondary_object().map(|secondary| secondary.into()),
    }
}

/// Reports an error coming from a controller to Kubernetes
///
/// This is inteded to be executed on the log entries returned by [`Controller::run`]
#[tracing::instrument(skip(client))]
pub fn publish_controller_error_as_k8s_event<ReconcileErr, QueueErr>(
    client: &Client,
    controller: &str,
    controller_error: &controller::Error<ReconcileErr, QueueErr>,
) where
    ReconcileErr: PublishableError,
    QueueErr: Error,
{
    let (error, obj) = match controller_error {
        controller::Error::ReconcilerFailed(err, obj) => (err, obj),
        // Other error types are intended for the operator administrator, and aren't linked to a specific object
        _ => return,
    };
    let recorder = Recorder::new(
        client.as_kube_client(),
        Reporter {
            controller: controller.to_string(),
            instance: None,
        },
        obj.clone().into(),
    );
    let event = error_to_event(error);
    // Run in the background
    tokio::spawn(
        async move {
            if let Err(err) = recorder.publish(event).await {
                tracing::error!(
                    error = &err as &dyn std::error::Error,
                    "Failed to report error as K8s event"
                );
            }
        }
        .in_current_span(),
    );
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::ConfigMap;
    use kube::runtime::reflector::ObjectRef;
    use strum_macros::EnumDiscriminants;

    use super::{error_to_event, PublishableError};

    #[derive(Debug, thiserror::Error, EnumDiscriminants)]
    #[strum_discriminants(derive(strum_macros::IntoStaticStr))]
    enum ErrorFoo {
        #[error("bar failed")]
        Bar { source: ErrorBar },
    }
    #[derive(Debug, thiserror::Error)]
    enum ErrorBar {
        #[error("baz failed")]
        Baz { source: ErrorBaz },
    }
    #[derive(Debug, thiserror::Error)]
    enum ErrorBaz {
        #[error("couldn't find chocolate")]
        NoChocolate { descriptor: ObjectRef<ConfigMap> },
    }
    impl ErrorFoo {
        fn no_chocolate() -> Self {
            Self::Bar {
                source: ErrorBar::Baz {
                    source: ErrorBaz::NoChocolate {
                        descriptor: ObjectRef::new("chocolate-descriptor").within("cupboard"),
                    },
                },
            }
        }
    }
    impl PublishableError for ErrorFoo {
        fn variant_name(&self) -> &'static str {
            ErrorFooDiscriminants::from(self).into()
        }

        fn secondary_object(&self) -> Option<ObjectRef<kube::core::DynamicObject>> {
            match self {
                ErrorFoo::Bar {
                    source:
                        ErrorBar::Baz {
                            source: ErrorBaz::NoChocolate { descriptor },
                        },
                } => Some(descriptor.clone().erase()),
            }
        }
    }

    #[test]
    fn event_should_report_full_nested_message() {
        let err = ErrorFoo::no_chocolate();
        assert_eq!(
            error_to_event(&err).note.as_deref(),
            Some("bar failed: baz failed: couldn't find chocolate")
        );
    }

    #[test]
    fn event_should_include_secondary_object() {
        let err = ErrorFoo::no_chocolate();
        let event = error_to_event(&err);
        let secondary = event.secondary.unwrap();
        assert_eq!(secondary.name.as_deref(), Some("chocolate-descriptor"));
        assert_eq!(secondary.namespace.as_deref(), Some("cupboard"));
        assert_eq!(secondary.kind.as_deref(), Some("ConfigMap"));
    }

    #[test]
    fn event_should_include_reason_code() {
        let err = ErrorFoo::no_chocolate();
        let event = error_to_event(&err);
        assert_eq!(event.reason, "Bar");
    }
}
