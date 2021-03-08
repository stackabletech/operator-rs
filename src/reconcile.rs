use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::{conditions, controller_ref, finalizer, podutils};

use crate::conditions::ConditionStatus;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, OwnerReference};
use kube::api::{ListParams, Meta, ObjectMeta};
use kube_runtime::controller::ReconcilerAction;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, info};

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
    pub async fn then<E>(
        self,
        next: impl Future<Output = Result<ReconcileFunctionAction, E>>,
    ) -> Result<ReconcileFunctionAction, E> {
        match self {
            ReconcileFunctionAction::Continue => next.await,
            action => Ok(action),
        }
    }
}

pub fn create_requeuing_reconcile_function_action(secs: u64) -> ReconcileFunctionAction {
    ReconcileFunctionAction::Requeue(Duration::from_secs(secs))
}

#[derive(Eq, PartialEq)]
pub enum DeletionStrategy {
    /// Will delete all illegal pods and continue with the reconciliation
    AllContinue,

    /// Will delete all illegal pods and requeue
    AllRequeue,

    /// Will delete just one illegal pod at a time and requeue
    OneRequeue,
}

pub struct ReconciliationContext<T> {
    pub client: Client,
    pub resource: T,
    pub requeue_timeout: Duration,
}

impl<T> ReconciliationContext<T> {
    pub fn new(client: Client, resource: T, requeue_timeout: Duration) -> Self {
        ReconciliationContext {
            client,
            resource,
            requeue_timeout,
        }
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

    pub async fn list_pods(&self) -> OperatorResult<Vec<Pod>> {
        let api = self.client.get_namespaced_api(&self.namespace());

        // TODO: In addition to filtering by OwnerReference (which can only be done client-side)
        // we could also add a custom label.

        // It'd be ideal if we could filter by ownerReferences but that's not possible in K8S today
        // so we apply a custom label to each pod
        let list_params = ListParams {
            label_selector: None,
            ..ListParams::default()
        };

        let owner_uid = self
            .resource
            .meta()
            .uid
            .as_ref()
            .ok_or(Error::MissingObjectKey {
                key: ".metadata.uid",
            })?;

        api.list(&list_params)
            .await
            .map_err(Error::from)
            .map(|result| result.items)
            .map(|pods| {
                pods.into_iter()
                    .filter(|pod| pod_owned_by(pod, owner_uid))
                    .collect()
            })
    }

    pub async fn handle_deletion(
        &self,
        handler: Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Error>> + Send + '_>>,
        finalizer: &str,
    ) -> ReconcileResult<Error>
    where
        T: Clone + DeserializeOwned + Meta + Send + Sync + 'static,
    {
        finalizer::add_finalizer(&self.client, &self.resource, finalizer).await?;

        if !finalizer::has_deletion_stamp(&self.resource) {
            debug!("Resource not deleted, continuing",);
            return Ok(ReconcileFunctionAction::Continue);
        }

        match handler.await? {
            ReconcileFunctionAction::Continue => Ok(ReconcileFunctionAction::Continue),
            ReconcileFunctionAction::Done => {
                info!("Removing finalizer [{}]", finalizer,);
                finalizer::remove_finalizer(&self.client, &self.resource, finalizer).await?;
                Ok(ReconcileFunctionAction::Done)
            }
            ReconcileFunctionAction::Requeue(_) => {
                Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout))
            }
        }
    }

    /// Creates a new [`Condition`] for the `resource` this context contains.
    ///
    /// It's a convenience function that passes through all parameters and builds a `Condition`
    /// using the [`conditions::build_condition`] method.
    pub fn build_condition_for_resource(
        &self,
        current_conditions: Option<&[Condition]>,
        message: String,
        reason: String,
        status: ConditionStatus,
        condition_type: String,
    ) -> Condition {
        conditions::build_condition(
            &self.resource,
            current_conditions,
            message,
            reason,
            status,
            condition_type,
        )
    }
}

// TODO: Trait bound on Clone is not needed after https://github.com/clux/kube-rs/pull/436
impl<T> ReconciliationContext<T>
where
    T: Clone + DeserializeOwned + Meta,
{
    /// Sets the [`Condition`] on the resource in this context.
    pub async fn set_condition(&self, condition: Condition) -> OperatorResult<T> {
        Ok(self.client.set_condition(&self.resource, condition).await?)
    }

    /// Builds a [`Condition`] using [`ReconciliationContext::build_condition_for_resource`] and then sets saves it.
    pub async fn build_and_set_condition(
        &self,
        current_conditions: Option<&[Condition]>,
        message: String,
        reason: String,
        status: ConditionStatus,
        condition_type: String,
    ) -> OperatorResult<T> {
        let condition = self.build_condition_for_resource(
            current_conditions,
            message,
            reason,
            status,
            condition_type,
        );
        self.set_condition(condition).await
    }

    /// Adds our finalizer to the list of finalizers.
    /// It is a wrapper around [`finalizer::add_finalizer`].
    /// Will either return an Error or [`ReconcileFunctionAction::Continue`].
    pub async fn add_finalizer(&self, finalizer_name: &str) -> ReconcileResult<Error> {
        // TODO: Add option to requeue?
        finalizer::add_finalizer(&self.client, &self.resource, finalizer_name).await?;
        Ok(ReconcileFunctionAction::Continue)
    }
}

/// This returns `false` for Pods that have no OwnerReference (with a Controller flag)
/// or where the Controller does not have the same `uid` as the passed in `owner_uid`.
/// If however the `uid` exists and matches we return `true`.
fn pod_owned_by(pod: &Pod, owner_uid: &str) -> bool {
    let controller = controller_ref::get_controller_of(pod);
    matches!(controller, Some(OwnerReference { uid, .. }) if uid == owner_uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pod_owned_by() {
        let mut pod = Pod {
            metadata: ObjectMeta {
                name: Some("Foobar".to_string()),
                owner_references: Some(vec![OwnerReference {
                    controller: Some(true),
                    uid: "1234-5678".to_string(),
                    ..OwnerReference::default()
                }]),
                ..ObjectMeta::default()
            },
            ..Pod::default()
        };

        assert!(pod_owned_by(&pod, "1234-5678"));

        pod.metadata.owner_references = None;
        assert!(!pod_owned_by(&pod, "1234-5678"));
    }
}
