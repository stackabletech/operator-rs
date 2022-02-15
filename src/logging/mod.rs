use kube::{
    runtime::{
        controller::{self, ReconcilerAction},
        reflector::ObjectRef,
    },
    Resource,
};
use tracing;
use tracing_subscriber::EnvFilter;

use crate::{client::Client, logging::k8s_events::publish_controller_error_as_k8s_event};

use self::k8s_events::PublishableError;

pub mod k8s_events;

/// Initializes `tracing` logging with options from the environment variable
/// given in the `env` parameter.
///
/// We force users to provide a variable name so it can be different per product.
/// We encourage it to be the product name plus `_LOG`, e.g. `FOOBAR_OPERATOR_LOG`.
/// If no environment variable is provided, the maximum log level is set to INFO.
pub fn initialize_logging(env: &str) {
    let filter = match EnvFilter::try_from_env(env) {
        Ok(env_filter) => env_filter,
        _ => EnvFilter::try_new(tracing::Level::INFO.to_string())
            .expect("Failed to initialize default tracing level to INFO"),
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Reports the controller reconciliation result to all relevant targets
pub fn report_controller_reconciled<K, ReconcileErr, QueueErr>(
    client: &Client,
    controller_name: &str,
    result: &Result<(ObjectRef<K>, ReconcilerAction), controller::Error<ReconcileErr, QueueErr>>,
) where
    K: Resource,
    ReconcileErr: PublishableError,
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
pub fn report_controller_error<ReconcileErr, QueueErr>(
    client: &Client,
    controller_name: &str,
    error: &controller::Error<ReconcileErr, QueueErr>,
) where
    ReconcileErr: PublishableError,
    QueueErr: std::error::Error,
{
    tracing::error!(
        controller.name = controller_name,
        error = &*error as &dyn std::error::Error,
        "Failed to reconcile object",
    );
    publish_controller_error_as_k8s_event(client, controller_name, error);
}

#[cfg(test)]
mod test {

    use tracing::{debug, error, info};

    // If there is a proper way to programmatically inspect the global max level than we should use that.
    // Until then, this is mostly a sanity check for the implementation above.
    // Either run
    //      cargo test default_tracing -- --nocapture
    // to see the ERROR and INFO messages, or
    //      NOT_SET=debug cargo test default_tracing -- --nocapture
    // to see them all.
    #[test]
    pub fn test_default_tracing_level_is_set_to_info() {
        super::initialize_logging("NOT_SET");

        error!("ERROR level messages should be seen.");
        info!("INFO level messages should also be seen by default.");
        debug!("DEBUG level messages should be seen only if you set the NOT_SET env var.");
    }
}
