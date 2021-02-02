use crate::client::Client;
use crate::error::Error;
use crate::podutils;

use k8s_openapi::api::core::v1::Pod;
use kube::api::{ListParams, Meta, ObjectMeta};
use kube_runtime::controller::ReconcilerAction;
use std::future::Future;
use std::time::Duration;

pub type ReconcileResult<E> = std::result::Result<ReconcileFunctionAction, E>;

/// Creates a [`ReconcilerAction`] that will trigger a requeue after a specified [`Duration`].
pub fn create_requeuing_reconciler_action(duration: Duration) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(duration),
    }
}

/// Creates a [`ReconcilerAction`] that won't trigger a requeue.
pub fn create_non_requeuing_reconciler_action() -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: None,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReconcileFunctionAction {
    /// Run the next function in the reconciler chain
    Continue,

    /// Skip the remaining reconciler chain
    Done,

    /// Skip the remaining reconciler chain and queue this object again
    Requeue(Duration),
}

impl ReconcileFunctionAction {
    pub async fn then(
        self,
        next: impl Future<Output = ReconcileFunctionAction>,
    ) -> ReconcileFunctionAction {
        match self {
            ReconcileFunctionAction::Continue => next.await,
            action => action,
        }
    }
}

pub fn create_requeuing_reconcile_function_action(secs: u64) -> ReconcileFunctionAction {
    ReconcileFunctionAction::Requeue(Duration::from_secs(secs))
}

pub struct ReconciliationContext<T> {
    pub client: Client,
    pub resource: T,
}

impl<T> ReconciliationContext<T> {
    pub fn new(client: Client, resource: T) -> Self {
        ReconciliationContext { client, resource }
    }
}

impl<T> ReconciliationContext<T>
where
    T: Meta,
{
    pub fn name(&self) -> String {
        Meta::name(&self.resource)
    }

    pub fn namespace(&self) -> String {
        Meta::namespace(&self.resource).expect("Resources are namespaced")
    }

    /// Returns a name that is suitable for directly passing to a log macro.
    ///
    /// See [`crate::podutils::get_log_name()`] for details.
    pub fn log_name(&self) -> String {
        podutils::get_log_name(&self.resource)
    }

    pub fn metadata(&self) -> ObjectMeta {
        self.resource.meta().clone()
    }

    pub async fn list_pods(&self) -> Result<Vec<Pod>, Error> {
        let api = self.client.get_namespaced_api(&self.namespace());

        // TODO: We need to use a label selector to only get _our_ pods
        // It'd be ideal if we could filter by ownerReferences but that's not possible in K8S today
        // so we apply a custom label to each pod
        let list_params = ListParams {
            label_selector: None,
            ..ListParams::default()
        };

        api.list(&list_params)
            .await
            .map_err(Error::from)
            .map(|result| result.items)
    }
}
