use crate::client::Client;
use crate::reconcile::{ReconcileFunctionAction, ReconciliationContext};

use crate::error::OperatorResult;
use crate::finalizer;
use futures::StreamExt;
use kube::api::{ListParams, Meta};
use kube::Api;
use kube_runtime::controller::{Context, ReconcilerAction};
use kube_runtime::Controller as KubeController;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use tokio::time::Duration;
use tracing::{debug, error, info, trace};

pub trait ControllerStrategy {
    type Item: Debug;
    type Error: Debug;
    type State: ReconciliationState + Debug;

    // TODO: type FutureType: ....? There are no trait bounds to apply here

    fn finalizer_name(&self) -> String;

    fn error_policy(&self) {
        // TODO: Pass in error
        // TODO: return ReconcilerAction?
        error!("Reconciliation error");
    }

    fn init_reconcile_state(&self, context: ReconciliationContext<Self::Item>) -> Self::State;
}

pub trait ReconciliationState {
    type Error: Debug;

    // The anonymous lifetime refers to the &self. So we could also rewrite this function signature
    // as `fn reconcile_operations<'a>(&'a self, .... >> + 'a>>>;` but that'd require every implementor
    // to also write all the lifetimes.
    // Just using the anonymous one makes it a bit easier.
    fn reconcile_operations(
        &self,
    ) -> Vec<Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + '_>>>;
}

/// A Controller is the object that watches all required resources and runs the reconciliation loop.
/// This struct wraps a [`kube_runtime::Controller`] and provides some comfort features.
///
/// To customize its behavior you need to provide a [`ControllerStrategy`].
///
/// * It automatically adds a finalizer to every new object
/// * It calls a method on the strategy for every error
/// * It calls a method on the strategy for every deleted resource so cleanup can happen
///   * It automatically removes the finalizer
/// * It calls a method for every _normal_ reconciliation run
pub struct Controller<T>
where
    T: Clone + DeserializeOwned + Meta + Send + Sync + 'static,
{
    kube_controller: KubeController<T>,
}

impl<T> Controller<T>
where
    T: Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
{
    pub fn new(api: Api<T>) -> Controller<T> {
        let controller = kube_runtime::Controller::new(api, ListParams::default());
        Controller {
            kube_controller: controller,
        }
    }

    /// Can be used to register additional watchers that will trigger a reconcile.
    ///
    /// If your main object creates further objects of differing types this can be used to get
    /// notified should one of those objects change.
    pub fn owns<Child: Clone + Meta + DeserializeOwned + Send + 'static>(
        mut self,
        api: Api<Child>,
        lp: ListParams,
    ) -> Self {
        self.kube_controller = self.kube_controller.owns(api, lp);
        self
    }

    pub async fn run<S>(self, client: Client, strategy: S)
    where
        S: ControllerStrategy<Item = T> + 'static,
        S::Error: std::fmt::Debug,
    {
        let context = Context::new(ControllerContext {
            client,
            strategy: Mutex::new(strategy),
        });

        self.kube_controller
            .run(reconcile, error_policy, context)
            .for_each(|res| async move {
                match res {
                    Ok(o) => info!("Reconciled {:?}", o),
                    Err(ref e) => error!("Reconcile failed: {:?}", e),
                };
            })
            .await
    }
}

/// The context used internally in the Controller which is passed on to the `kube_runtime::Controller`.
struct ControllerContext<S> {
    client: Client,
    strategy: Mutex<S>,
}

fn create_reconciler_action() -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: None,
    }
}

/// This method contains the logic of reconciling an object (the desired state) we received with the actual state.
async fn reconcile<S, T>(
    resource: T,
    context: Context<ControllerContext<S>>,
) -> Result<ReconcilerAction, crate::error::Error>
where
    T: Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
    S: ControllerStrategy<Item = T> + 'static,
    S::Error: std::fmt::Debug,
{
    let context = context.get_ref();
    let strategy = context.strategy.lock().unwrap();

    if handle_deletion(
        &resource,
        context.client.clone(),
        &strategy.finalizer_name(),
    )
    .await?
        == ReconcileFunctionAction::Done
    {
        return Ok(create_reconciler_action());
    }

    add_finalizer(
        &resource,
        context.client.clone(),
        &strategy.finalizer_name(),
    )
    .await?;

    let rc = ReconciliationContext::new(context.client.clone(), resource.clone());

    let state = strategy.init_reconcile_state(rc);
    let futures = state.reconcile_operations();

    for future in futures {
        let result = future.await;

        match result {
            Ok(ReconcileFunctionAction::Continue) => {
                trace!("Reconciler loop: Continue");
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

fn error_policy<S, E>(error: &E, _: Context<ControllerContext<S>>) -> ReconcilerAction
where
    E: std::fmt::Display,
{
    error!("Reconciliation error:\n{}", error);
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(10)),
    }
}

async fn handle_deletion<T>(
    resource: &T,
    client: Client,
    finalizer_name: &str,
) -> OperatorResult<ReconcileFunctionAction>
where
    T: Clone + DeserializeOwned + Meta + Send + Sync + 'static,
{
    let address = format!("[{:?}/{}]", Meta::namespace(resource), Meta::name(resource));
    trace!("Reconciler [handle_deletion] for {}", address);
    if !finalizer::has_deletion_stamp(resource) {
        debug!(
            "[handle_deletion] for {}: Not deleted, continuing...",
            address
        );
        return Ok(ReconcileFunctionAction::Continue);
    }

    info!("Deleting resource {}", address);
    finalizer::remove_finalizer(client, resource, finalizer_name).await?;

    Ok(ReconcileFunctionAction::Done)
}

async fn add_finalizer<T>(resource: &T, client: Client, finalizer_name: &str) -> OperatorResult<()>
where
    T: Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
{
    let address = format!("[{:?}/{}]", Meta::namespace(resource), Meta::name(resource));
    trace!(resource = ?resource, "Reconciler [add_finalizer] for {}", address);

    if finalizer::has_finalizer(resource, finalizer_name) {
        debug!(
            "[add_finalizer] for {}: Finalizer already exists, continuing...",
            address
        );
    } else {
        debug!(
            "[add_finalizer] for {}: Finalizer missing, adding now and continuing...",
            address
        );
        finalizer::add_finalizer(client, resource, finalizer_name).await?;
    }
    Ok(())
}
