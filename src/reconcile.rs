use crate::client::Client;
use crate::error::Error;
use futures::future::BoxFuture;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{ListParams, Meta, ObjectMeta};
use kube_runtime::controller::ReconcilerAction;
use std::time::Duration;
use tracing::{error, trace};

/// Functions running as part of the [`run_reconcile_functions`] loop must match this signature
pub type ReconcileFunction<C, T, E> =
    fn(&mut ReconciliationContext<C, T>) -> BoxFuture<'_, ReconcileResult<E>>;

pub type ReconcileResult<E> = std::result::Result<ReconcileFunctionAction, E>;

pub enum ReconcileFunctionAction {
    /// Run the next [`ReconcileFunction`]
    Continue,

    /// Skip the remaining [`ReconcileFunction`]s
    Done,

    /// Skip the remaining [`ReconcileFunction`]s and queue this object again
    Requeue(Duration),
}

pub struct ReconciliationContext<C, T> {
    pub client: Client,
    pub resource: T,
    pub context: Option<C>,
}

impl<C, T> ReconciliationContext<C, T> {
    pub fn new(client: Client, resource: T) -> Self {
        ReconciliationContext {
            client,
            resource,
            context: None,
        }
    }
}

impl<C, T> ReconciliationContext<C, T>
where
    T: Meta,
{
    pub fn name(&self) -> String {
        Meta::name(&self.resource)
    }

    pub fn namespace(&self) -> String {
        Meta::namespace(&self.resource).expect("Resources are namespaced")
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

pub async fn run_reconcile_functions<C, T, E>(
    reconcilers: &[ReconcileFunction<C, T, E>],
    context: &mut ReconciliationContext<C, T>,
) -> Result<ReconcilerAction, E>
where
    E: std::fmt::Debug,
{
    for reconciler in reconcilers {
        match reconciler(context).await {
            Ok(ReconcileFunctionAction::Continue) => {
                trace!("Reconciler loop: Continue")
            }
            Ok(ReconcileFunctionAction::Done) => {
                trace!("Reconciler loop: Done");
                break;
            }
            Ok(ReconcileFunctionAction::Requeue(duration)) => {
                trace!(?duration, "Reconciler loop: Requeue");
                return Ok(ReconcilerAction {
                    requeue_after: Some(duration),
                });
            }
            Err(err) => {
                error!(?err, "Error reconciling");
                return Ok(ReconcilerAction {
                    requeue_after: Some(Duration::from_secs(30)),
                });
            }
        }
    }

    Ok(ReconcilerAction {
        requeue_after: None,
    })
}
