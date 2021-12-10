//! A way to implement a Kubernetes Operator on top of the [kube-rs](https://github.com/clux/kube-rs) library.
//!
//! This is an opinionated wrapper around the [`kube::runtime::Controller`] from said library.
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
//! use async_trait::async_trait;
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
//! #[async_trait]
//! impl ControllerStrategy for FooStrategy {
//!     type Item = Pod;
//!     type State = FooState;
//!     type Error = String;
//!
//!     async fn init_reconcile_state(&self,context: ReconciliationContext<Self::Item>) -> Result<Self::State, Self::Error> {
//!         Ok(FooState {
//!             my_state: 1
//!         })
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
//!     controller.run(client, strategy, Duration::from_secs(10)).await;
//! }
//! ```
//!
use crate::client::Client;
use crate::error::Error;
#[allow(deprecated)]
use crate::reconcile;
#[allow(deprecated)]
use crate::reconcile::{ReconcileFunctionAction, ReconciliationContext};

use async_trait::async_trait;
use futures::StreamExt;
use kube::api::ListParams;
use kube::runtime::controller::{Context, ReconcilerAction};
use kube::runtime::Controller as KubeController;
use kube::{Api, Resource};
use serde::de::DeserializeOwned;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, error, trace, warn, Instrument};
use uuid::Uuid;

/// Every operator needs to provide an implementation of this trait as it provides the operator specific business logic.
#[async_trait]
#[allow(deprecated)]
pub trait ControllerStrategy {
    type Item;
    type State: ReconciliationState;
    type Error: Debug;

    // TODO: Pass in error: https://github.com/stackabletech/operator-rs/issues/122
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
    async fn init_reconcile_state(
        &self,
        context: ReconciliationContext<Self::Item>,
    ) -> Result<Self::State, Self::Error>;
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
    #[allow(deprecated)]
    fn reconcile(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + Send + '_>>;
}

pub trait HasOwned {
    fn owned_objects() -> Vec<&'static str>;
}

/// A Controller is the object that watches all required resources and runs the reconciliation loop.
/// This struct wraps a [`kube::runtime::Controller`] and provides some comfort features.
///
/// A single Controller always has one _main_ resource type it watches for but you can add
/// additional resource types via the `owns` method but those only trigger a reconciliation run if
/// they have an `OwnerReference` that matches one of the main resources.
/// This `OwnerReference` currently needs to be set manually!
///
/// To customize the behavior of the Controller you need to provide a [`ControllerStrategy`].
///
/// * It calls a method on the strategy for every error
///   * It automatically removes the finalizer
/// * It creates (via the Strategy) a [`ReconciliationState`] object for every reconciliation and
///   calls its [`ReconciliationState::reconcile`] method to get a list of operations (Futures) to run
///   * It then proceeds to poll all those futures serially until one of them does not return `Continue`
pub struct Controller<T>
where
    T: Clone + Debug + DeserializeOwned + Resource + Send + Sync + 'static,
    <T as Resource>::DynamicType: Debug + Eq + Hash,
{
    kube_controller: KubeController<T>,
}

impl<T> Controller<T>
where
    T: Clone + Debug + DeserializeOwned + Resource + Send + Sync + 'static,
    <T as Resource>::DynamicType: Clone + Debug + Default + Eq + Hash + Unpin,
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
    pub fn owns<Child>(mut self, api: Api<Child>, lp: ListParams) -> Self
    where
        Child: Clone + Resource<DynamicType = ()> + DeserializeOwned + Debug + Send + 'static,
    {
        self.kube_controller = self.kube_controller.owns(api, lp);
        self
    }

    /// Call this method once your Controller object is fully configured to start the reconciliation.
    ///
    /// # Arguments
    ///
    /// * `client` - The Client to access Kubernetes
    /// * `strategy` - This implements the domain/business logic and the framework will call its methods for each reconcile operation
    /// * `requeue_timeout` - Whenever a `Requeue` is returned this is the timeout/duration after which the same object will be requeued
    pub async fn run<S>(self, client: Client, strategy: S, requeue_timeout: Duration)
    where
        S: ControllerStrategy<Item = T> + Send + Sync + 'static,
        S::State: Send,
    {
        let context = Context::new(ControllerContext {
            client,
            strategy,
            requeue_timeout,
        });

        self.kube_controller
            .run(reconcile, error_policy, context)
            .for_each(|res| async move {
                match res {
                    Ok(o) => trace!(resource = ?o, "Reconciliation finished successfully (it is normal to see this message twice)"),
                    Err(err @ kube::runtime::controller::Error::ObjectNotFound {..}) => {
                        // An object may have been deleted after it was scheduled (but before it was executed). This is typically not an error.
                        trace!(err = &err as &(dyn std::error::Error + 'static), "ObjectNotFound in store, this is normal and will be retried")
                    },
                    Err(err @ kube::runtime::controller::Error::QueueError(kube::runtime::watcher::Error::WatchFailed {..})) => {
                        // This can happen when we lose the connection to the apiserver or the
                        // connection gets interrupted for any other reason.
                        // kube-rs will usually try to restart the watch automatically.
                        warn!(err = &err as &(dyn std::error::Error + 'static), "controller watch failed, will retry")
                    },
                    Err(err) => {
                        // If we get here it means that our `reconcile` method returned an error which should never happen because
                        // we convert all errors to requeue operations.
                        error!(err = &err as &(dyn std::error::Error + 'static), "Reconciliation finished with an error, this should not happen, please file an issue")
                    }
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
    requeue_timeout: Duration,
}

/// This method contains the logic of reconciling an object (the desired state) we received with the actual state.
#[tracing::instrument(
    skip(resource, context),
    fields(request_id = %Uuid::new_v4()),
)]
#[allow(deprecated)]
async fn reconcile<S, T>(
    resource: T,
    context: Context<ControllerContext<S>>,
) -> Result<ReconcilerAction, Error>
where
    T: Clone + Debug + DeserializeOwned + Resource + Send + Sync + 'static,
    S: ControllerStrategy<Item = T> + 'static,
{
    debug!(?resource, "Beginning reconciliation");
    let context = context.get_ref();

    let strategy = &context.strategy;

    let rc = ReconciliationContext::new(
        context.client.clone(),
        resource.clone(),
        context.requeue_timeout,
    );

    let mut state = match strategy.init_reconcile_state(rc).in_current_span().await {
        Ok(state) => state,
        Err(err) => {
            error!(
                ?err,
                "Error initializing reconciliation state, will requeue"
            );
            return Ok(ReconcilerAction {
                // TODO: Make this configurable https://github.com/stackabletech/operator-rs/issues/124
                requeue_after: Some(context.requeue_timeout),
            });
        }
    };
    let result = state.reconcile().in_current_span().await;
    match result {
        Ok(ReconcileFunctionAction::Requeue(duration)) => {
            trace!(
                action = "Requeue",
                ?duration,
                "Reconciliation finished successfully (it is normal to see this message twice)"
            );
            Ok(ReconcilerAction {
                requeue_after: Some(duration),
            })
        }
        Ok(action) => {
            trace!(
                ?action,
                "Reconciliation finished successfully (it is normal to see this message twice)"
            );
            Ok(ReconcilerAction {
                requeue_after: None,
            })
        }
        Err(err) => {
            error!(?err, "Reconciliation finished with an error, will requeue");
            Ok(ReconcilerAction {
                requeue_after: Some(context.requeue_timeout),
            })
        }
    }
}

fn error_policy<S, E>(err: &E, context: Context<ControllerContext<S>>) -> ReconcilerAction
where
    E: Display,
    S: ControllerStrategy,
{
    trace!(%err, "Reconciliation error, calling strategy error_policy");
    context.get_ref().strategy.error_policy()
}
