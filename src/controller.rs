use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::reconcile::{ReconcileFunctionAction, ReconciliationContext};
use crate::{finalizer, reconcile};

use futures::StreamExt;
use kube::api::{ListParams, Meta};
use kube::Api;
use kube_runtime::controller::{Context, ReconcilerAction};
use kube_runtime::Controller as KubeController;
use serde::de::DeserializeOwned;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, error, info, trace};

/// Every operator needs to provide an implementation of this trait as it provides the operator specific logic.
pub trait ControllerStrategy {
    type Item;
    type State: ReconciliationState;

    fn finalizer_name(&self) -> String;

    fn error_policy(&self) -> ReconcilerAction {
        // TODO: Pass in error
        // TODO: return ReconcilerAction?
        error!("Reconciliation error");
        reconcile::create_requeuing_reconciler_action(Duration::from_secs(30))
    }

    /// This is being called for each new reconciliation run.
    ///
    /// It provides a context with the _main_ resource (not necessarily the one that triggered
    /// this reconciliation run) as well as a client to access Kubernetes.
    ///
    /// It needs to return another struct that needs to implement [`ReconciliationState`].
    /// The idea is that every reconciliation run has its own state and it is not shared
    /// between runs.
    fn init_reconcile_state(&self, context: ReconciliationContext<Self::Item>) -> Self::State;
}

pub trait ReconciliationState {
    /// The associated error which can be returned from the reconciliation operations.
    type Error: Debug;

    // The anonymous lifetime refers to the &self. So we could also rewrite this function signature
    // as `fn reconcile_operations<'a>(&'a self, .... >> + 'a>>>;` but that'd require every implementor
    // to also write all the lifetimes.
    // Just using the anonymous one makes it a bit easier.
    // Choosing this lifetime instead of 'static was deliberate because it allows us to return Futures
    // that take a `self` argument and the Controller is the owner of the `ReconciliationState` object
    // so this should work just fine.
    //
    /// Provides a list of futures all taking no arguments and returning a [`ReconcileFunctionAction`].
    /// The controller will call them in order until one of them does _not_ return `Continue`.
    fn reconcile_operations(
        &self,
    ) -> Vec<Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + '_>>>;
}

/// A Controller is the object that watches all required resources and runs the reconciliation loop.
/// This struct wraps a [`kube_runtime::Controller`] and provides some comfort features.
///
/// A single Controller always has one _main_ resource type it watches for but you can add
/// additional resource types via the `owns` method but those only trigger a reconciliation run if
/// they have an `OwnerReference` that matches one of the main resources.
/// This `OwnerReference` currently needs to be set manually!
///
/// To customize the behavior of the Controller you need to provide a [`ControllerStrategy`].
///
/// * It automatically adds a finalizer to every new _main_ object
///   * If you need one on _owned_ objects you currently need to handle this yourself
/// * It calls a method on the strategy for every error
/// * TODO It calls a method on the strategy for every deleted resource so cleanup can happen
///   * It automatically removes the finalizer
/// * It creates (via the Strategy) a [`ReconciliationState`] object for every reconciliation and
///   calls its [`ReconciliationState::reconcile_operations`] method to get a list of operations (Futures) to run
///   * It then proceeds to poll all those futures serially until one of them does not return `Continue`
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
        let controller = KubeController::new(api, ListParams::default());
        Controller {
            kube_controller: controller,
        }
    }

    /// Can be used to register additional watchers that will trigger a reconcile.
    ///
    /// If your main object creates further objects of differing types this can be used to get
    /// notified should one of those objects change.
    ///
    /// Only objects that have an `OwnerReference` for our main resource type will trigger
    /// a reconciliation.
    /// You need to make sure to add this `OwnerReference` yourself.
    pub fn owns<Child: Clone + Meta + DeserializeOwned + Send + 'static>(
        mut self,
        api: Api<Child>,
        lp: ListParams,
    ) -> Self {
        self.kube_controller = self.kube_controller.owns(api, lp);
        self
    }

    /// Call this method once your Controller object is fully configured.
    /// It'll start talking to Kubernetes and will call the `Strategy` implementation.
    pub async fn run<S>(self, client: Client, strategy: S)
    where
        S: ControllerStrategy<Item = T> + 'static,
    {
        let context = Context::new(ControllerContext { client, strategy });

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
/// Note that we can only get immutable references to this object so should we need mutability we need to model it as interior mutability (e.g. with a Mutex)
struct ControllerContext<S>
where
    S: ControllerStrategy,
{
    client: Client,
    strategy: S,
}

/// This method contains the logic of reconciling an object (the desired state) we received with the actual state.
async fn reconcile<S, T>(
    resource: T,
    context: Context<ControllerContext<S>>,
) -> Result<ReconcilerAction, Error>
where
    T: Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
    S: ControllerStrategy<Item = T> + 'static,
{
    let context = context.get_ref();

    let client = &context.client;
    let strategy = &context.strategy;

    if handle_deletion(&resource, client.clone(), &strategy.finalizer_name()).await?
        == ReconcileFunctionAction::Done
    {
        return Ok(reconcile::create_non_requeuing_reconciler_action());
    }

    add_finalizer(&resource, client.clone(), &strategy.finalizer_name()).await?;

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
                return Ok(reconcile::create_requeuing_reconciler_action(
                    Duration::from_secs(30),
                ));
                // TODO: Make configurable
            }
        }
    }

    Ok(ReconcilerAction {
        requeue_after: None,
    })
}

// TODO: Properly type the error so we can pass it along
fn error_policy<S, E>(error: &E, context: Context<ControllerContext<S>>) -> ReconcilerAction
where
    E: Display,
    S: ControllerStrategy,
{
    trace!(
        "Reconciliation error, calling strategy error_policy:\n{}",
        error
    );
    context.get_ref().strategy.error_policy()
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

    info!(
        "Removing finalizer [{}] for resource {}",
        finalizer_name, address
    );
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
