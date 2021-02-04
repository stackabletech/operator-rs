//! A way to implement a Kubernetes Operator on top of the [kube-rs](https://github.com/clux/kube-rs) library.
//!
//! This is an opinionated wrapper around the [`kube_runtime::Controller`] from said library.
//! The idea is that a single reconcile run can _and should_ be separated into lots of small steps
//! each taking the reconciliation one step towards a stable state.
//!
//! Every time the Operator changes any visible state in Kubernetes it should immediately return and requeue the resource.
//! The _requeue_ can happen immediately or after a custom duration.
//! The latter can be useful for operations that are expected to take a while (e.g. creating a resource which is under control by a different Operator).
//!
//! This module has a [`Controller`] which is the main object that drives the reconciliation loop.
//! Custom logic needs to be plugged in by implementing a [`ControllerStrategy`] trait.
//! The `Controller` will call `init_reconcile_state(...)` on the `ControllerStrategy` for every reconciliation that is triggered.
//! This method returns a `ReconciliationState` trait which can contain arbitrary state about this
//! specific reconciliation run.
//!
//! Once the state object has been created its `reconcile` method is called.
//! The `reconcile` method is where the business logic for each operator gets called.
//! `ReconcileFunctionAction`s can be used to chain multiple calls to separate methods.
//! By using the `then` method on `ReconcileFunctionAction` we can automatically abort and/or requeue
//! in case of an `Error` or a `ReconciliationFunctionAction::Requeue(duration)`.
//!
//! See the example below for how to use this abstraction in a real-world operator.
//!
//! # Example
//!
//! ```no_run
//! use kube::Api;
//! use k8s_openapi::api::core::v1::Pod;
//! use stackable_operator::client;
//! use stackable_operator::controller::{Controller, ControllerStrategy, ReconciliationState};
//! use stackable_operator::reconcile::{ReconciliationContext, ReconcileFunctionAction, ReconcileResult};
//! use std::pin::Pin;
//! use std::future::Future;
//! use std::time::Duration;
//!
//! struct FooStrategy {
//! }
//!
//! struct FooState {
//!     my_state: i32
//! }
//!
//! type FooReconcileResult = ReconcileResult<String>;
//!
//! impl FooState {
//!     async fn test1(&mut self) -> FooReconcileResult {
//!         self.my_state = 123;
//!         println!("My reconciliation logic part 1");
//!         Ok(ReconcileFunctionAction::Continue)
//!     }
//!
//!     async fn test2(&self) -> FooReconcileResult {
//! println!("My reconciliation logic part 2");
//!         if self.my_state > 100 {
//!             return Ok(ReconcileFunctionAction::Requeue(Duration::from_secs(10)));
//!         }   
//!         Ok(ReconcileFunctionAction::Continue)
//!     }
//!
//!     async fn test3(&self) -> FooReconcileResult {
//! println!("My reconciliation logic part 3");
//!         if self.my_state > 100 {
//!             return Ok(ReconcileFunctionAction::Done);
//!         }   
//!         Ok(ReconcileFunctionAction::Continue)
//!     }
//! }
//!
//! impl ReconciliationState for FooState {
//!     type Error = String;
//!
//!     fn reconcile(
//!         &mut self,
//!     ) -> Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + Send + '_>>
//!     {
//!         Box::pin(async move {
//!             self.test1()
//!                 .await?
//!                 .then(self.test2())
//!                 .await?
//!                 .then(self.test3())
//!                 .await
//!         })
//!     }
//!     
//! }
//!
//! impl ControllerStrategy for FooStrategy {
//!     type Item = Pod;
//!     type State = FooState;
//!
//!     fn finalizer_name(&self) -> String {
//!         "foo.stackable.de/finalizer".to_string()
//!     }
//!
//!     fn init_reconcile_state(&self,context: ReconciliationContext<Self::Item>) -> Self::State {
//!         FooState {
//!             my_state: 1
//!         }
//!     }     
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = client::create_client(None).await.unwrap();
//!     let pods_api: Api<Pod> = client.get_all_api();
//!
//!     let controller = Controller::new(pods_api);
//!
//!     let strategy = FooStrategy {};
//!     controller.run(client, strategy).await;
//! }
//! ```
//!
use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::reconcile::{ReconcileFunctionAction, ReconciliationContext};
use crate::{finalizer, podutils, reconcile};

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

/// Every operator needs to provide an implementation of this trait as it provides the operator specific business logic.
pub trait ControllerStrategy {
    type Item;
    type State: ReconciliationState;

    fn finalizer_name(&self) -> String;

    // TODO: Pass in error
    fn error_policy(&self) -> ReconcilerAction {
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
    // to also write out all the lifetimes.
    // Just using the anonymous one makes it a bit easier to read albeit less explicit.
    //
    // Choosing this anonymous lifetime instead of 'static was deliberate.
    // It ties the lifetime of the returned Future to the `self` that we pass in.
    // This is often not desired and `static is easier to use because then the returned Future is fully owned by the caller.
    // In this case the `self` (i.e. the `ReconciliationState`) is fully owned by the caller (the `Controller`) and it never escapes.
    //
    // TODO: I'm not sure but can we maybe use async-trait and just make this function async and return the Result<...> directly?
    //
    /// Returns a Future that - when completed - will either return an `Error` or a `ReconciliationFunctionAction`.
    /// The return result can be used to requeue the same resource for later.
    fn reconcile(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + Send + '_>>;
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
        S: ControllerStrategy<Item = T> + Send + Sync + 'static,
        S::State: Send,
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

    let mut state = strategy.init_reconcile_state(rc);
    let result = state.reconcile().await;
    match result {
        Ok(ReconcileFunctionAction::Requeue(duration)) => {
            trace!(?duration, "Reconciler loop: Requeue");
            return Ok(ReconcilerAction {
                requeue_after: Some(duration),
            });
        }
        Ok(action) => {
            trace!("Reconciler loop: {:?}", action);
        }
        Err(err) => {
            error!("Error reconciling [{:?}]", err);
            return Ok(ReconcilerAction {
                // TODO: Make this configurable
                requeue_after: Some(Duration::from_secs(30)),
            });
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
    trace!(
        "Reconciler [handle_deletion] for {}",
        podutils::get_log_name(resource)
    );
    if !finalizer::has_deletion_stamp(resource) {
        debug!(
            "[handle_deletion] for {}: Not deleted, continuing...",
            podutils::get_log_name(resource)
        );
        return Ok(ReconcileFunctionAction::Continue);
    }

    info!(
        "Removing finalizer [{}] for resource {}",
        finalizer_name,
        podutils::get_log_name(resource)
    );
    finalizer::remove_finalizer(client, resource, finalizer_name).await?;

    Ok(ReconcileFunctionAction::Done)
}

async fn add_finalizer<T>(resource: &T, client: Client, finalizer_name: &str) -> OperatorResult<()>
where
    T: Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
{
    trace!(resource = ?resource, "Reconciler [add_finalizer] for {}", podutils::get_log_name(resource));

    if finalizer::has_finalizer(resource, finalizer_name) {
        debug!(
            "[add_finalizer] for {}: Finalizer already exists, continuing...",
            podutils::get_log_name(resource)
        );
    } else {
        debug!(
            "[add_finalizer] for {}: Finalizer missing, adding now and continuing...",
            podutils::get_log_name(resource)
        );
        finalizer::add_finalizer(client, resource, finalizer_name).await?;
    }
    Ok(())
}
