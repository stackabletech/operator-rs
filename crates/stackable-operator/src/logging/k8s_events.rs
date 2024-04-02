//! Utilities for publishing Kubernetes events

use std::error::Error;

use crate::client::Client;
use kube::runtime::{
    controller,
    events::{Event, EventType, Recorder, Reporter},
};
use tracing::Instrument;

use super::controller::ReconcilerError;

/// Converts an [`Error`] into a publishable Kubernetes [`Event`]
fn error_to_event<E: ReconcilerError>(err: &E) -> Event {
    // Walk the whole error chain, so that we get all the full reason for the error
    let mut full_msg = {
        use std::fmt::Write;
        let mut buf = err.to_string();
        let mut err: &dyn Error = err;
        loop {
            err = match err.source() {
                Some(err) => {
                    write!(buf, ": {err}").unwrap();
                    err
                }
                None => break buf,
            }
        }
    };
    message::truncate_with_ellipsis(&mut full_msg, 1024);
    Event {
        type_: EventType::Warning,
        reason: err.category().to_string(),
        note: Some(full_msg),
        action: "Reconcile".to_string(),
        secondary: err.secondary_object().map(|secondary| secondary.into()),
    }
}

/// Reports an error coming from a controller to Kubernetes
///
/// This is inteded to be executed on the log entries returned by [`kube::runtime::Controller::run`]
#[tracing::instrument(skip(client))]
pub fn publish_controller_error_as_k8s_event<ReconcileErr, QueueErr>(
    client: &Client,
    controller: &str,
    controller_error: &controller::Error<ReconcileErr, QueueErr>,
) where
    ReconcileErr: ReconcilerError,
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

mod message {
    /// Ensures that `msg` is at most `max_len` _bytes_ long
    ///
    /// If `msg` is longer than `max_len` then the extra text is replaced with an ellipsis.
    pub fn truncate_with_ellipsis(msg: &mut String, max_len: usize) {
        const ELLIPSIS: char = '…';
        const ELLIPSIS_LEN: usize = ELLIPSIS.len_utf8();
        let len = msg.len();
        if len > max_len {
            let start_of_trunc_char = find_start_of_char(msg, max_len.saturating_sub(ELLIPSIS_LEN));
            msg.truncate(start_of_trunc_char);
            if ELLIPSIS_LEN <= max_len {
                msg.push(ELLIPSIS);
            }
        }
        debug_assert!(msg.len() <= max_len);
    }

    fn find_start_of_char(s: &str, mut pos: usize) -> usize {
        loop {
            if s.is_char_boundary(pos) {
                break pos;
            }
            pos -= 1;
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::logging::k8s_events::message::find_start_of_char;

        use super::truncate_with_ellipsis;

        #[test]
        fn truncate_should_be_noop_if_string_fits() {
            let mut x = "hello".to_string();
            truncate_with_ellipsis(&mut x, 5);
            assert_eq!(&x, "hello");
        }

        #[test]
        fn truncate_should_ellipsize_large_string() {
            let mut x = "hello".to_string();
            truncate_with_ellipsis(&mut x, 4);
            assert_eq!(&x, "h…");
            x = "hello, this is a much larger string".to_string();
            truncate_with_ellipsis(&mut x, 4);
            assert_eq!(&x, "h…");
        }

        #[test]
        fn truncate_should_ellipsize_emoji() {
            let mut x = "hello🙋".to_string();
            truncate_with_ellipsis(&mut x, 8);
            assert_eq!(&x, "hello…");
        }

        #[test]
        fn find_start_of_char_should_be_noop_for_ascii() {
            assert_eq!(find_start_of_char("hello", 2 /* l */), 2);
        }

        #[test]
        fn find_start_of_char_should_find_start_of_emoji() {
            assert_eq!(
                find_start_of_char("hello🙋", 7 /* in the middle of the emoji */),
                5
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::ConfigMap;
    use kube::runtime::reflector::ObjectRef;
    use snafu::Snafu;
    use strum::EnumDiscriminants;

    use super::{error_to_event, ReconcilerError};

    #[derive(Snafu, Debug, EnumDiscriminants)]
    #[strum_discriminants(derive(strum::IntoStaticStr))]
    enum ErrorFoo {
        #[snafu(display("bar failed"))]
        Bar { source: ErrorBar },
    }
    #[derive(Snafu, Debug)]
    enum ErrorBar {
        #[snafu(display("baz failed"))]
        Baz { source: ErrorBaz },
    }
    #[derive(Snafu, Debug)]
    enum ErrorBaz {
        #[snafu(display("couldn't find chocolate"))]
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
    impl ReconcilerError for ErrorFoo {
        fn category(&self) -> &'static str {
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
