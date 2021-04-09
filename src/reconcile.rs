use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::k8s_utils::LabelOptionalValueMap;
use crate::{conditions, controller_ref, finalizer, labels, pod_utils};

use crate::conditions::ConditionStatus;
use crate::k8s_utils::find_excess_pods;
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector, OwnerReference};
use kube::api::{Meta, ObjectMeta};
use kube_runtime::controller::ReconcilerAction;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, info, trace, warn};

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
    /// Can be used to chain multiple functions which all return a Result<ReconcileFunctionAction, E>.
    ///
    /// Will call the `next` function in the chain only if the previous returned `Continue`.
    /// Will return the result from the last one otherwise.
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

#[derive(Eq, PartialEq)]
pub enum ContinuationStrategy {
    /// Will process all resources (including potential changes) and then continue with the reconciliation
    AllContinue,

    /// Will process all resources (including potential changes) and then requeue the resource
    AllRequeue,

    /// Will process all resources but will return a requeue after any changes
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

    fn requeue(&self) -> ReconcileFunctionAction {
        ReconcileFunctionAction::Requeue(self.requeue_timeout)
    }

    /// This is a reconciliation gate to wait for a list of Pods to be running and ready.
    ///
    /// See [`podutils::is_pod_running_and_ready`] for details.
    /// Will requeue as soon as a single Pod is not running or not ready.
    pub async fn wait_for_running_and_ready_pods(&self, pods: &[Pod]) -> ReconcileResult<Error> {
        wait_for_running_and_ready_pods(&self.requeue_timeout, pods)
    }

    /// This is a reconciliation gate to wait for a list of Pods to terminate.
    ///
    /// Will requeue as soon as a single Pod is in the process of terminating.
    pub async fn wait_for_terminating_pods(&self, pods: &[Pod]) -> ReconcileResult<Error> {
        wait_for_terminating_pods(&self.requeue_timeout, pods)
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
        pod_utils::get_log_name(&self.resource)
    }

    pub fn metadata(&self) -> ObjectMeta {
        self.resource.meta().clone()
    }

    /// This lists all Pods that have an OwnerReference that points to us (the object from `self.resource`)
    /// as its Controller.
    ///
    /// Unfortunately the Kubernetes API does _not_ allow filtering by OwnerReference so we have to fetch
    /// all Pods and filter them on the client.
    /// To reduce this overhead a LabelSelector will be included that uses the standard
    /// `app.kubernetes.io/instance` label and will use the name of the resource in this context
    /// as its value.
    /// You need to make sure to always set this label correctly!
    pub async fn list_pods(&self) -> OperatorResult<Vec<Pod>> {
        let owner_uid = self
            .resource
            .meta()
            .uid
            .as_ref()
            .ok_or(Error::MissingObjectKey {
                key: ".metadata.uid",
            })?;

        let mut labels = BTreeMap::new();
        labels.insert(labels::APP_INSTANCE_LABEL.to_string(), self.resource.name());

        let label_selector = LabelSelector {
            match_expressions: None,
            match_labels: Some(labels),
        };

        self.client
            .list_with_label_selector(self.resource.namespace().as_deref(), &label_selector)
            .await
            .map(|pods| {
                pods.into_iter()
                    .filter(|pod| pod_owned_by(pod, owner_uid))
                    .collect()
            })
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

    /// Checks all passed Pods to see if they fulfil some basic requirements.
    ///
    /// * They need to have all required labels and optionally one of a list of allowed values
    /// * They need to have a spec.node_name
    /// * TODO: Should check for all app.kubernetes.io labels
    ///
    /// If not they are considered invalid and will be deleted.
    ///
    /// This is a safety measure and should never actually delete any Pods as all Pods operators create
    /// should obviously all be valid.
    /// If this ever deletes a Pod it'll be either a programming error or a user who created or changed
    /// Pods manually.
    ///
    /// Implementation note: Unfortunately the required label structure is slightly different here than in `delete_excess_pods`
    /// and while that one could be converted into the one we need it'd require another parameter
    /// to ignore certain labels (e.g. `role group` values should never be checked)
    pub async fn delete_illegal_pods(
        &self,
        pods: &[Pod],
        required_labels: &BTreeMap<String, Option<Vec<String>>>,
        deletion_strategy: ContinuationStrategy,
    ) -> ReconcileResult<Error> {
        let illegal_pods = pod_utils::find_invalid_pods(pods, required_labels);
        if illegal_pods.is_empty() {
            return Ok(ReconcileFunctionAction::Continue);
        }

        for illegal_pod in illegal_pods {
            warn!(
                "Deleting invalid Pod [{}]",
                pod_utils::get_log_name(illegal_pod)
            );
            self.client.delete(illegal_pod).await?;

            if deletion_strategy == ContinuationStrategy::OneRequeue {
                trace!(
                    "Will requeue after deleting an illegal pod, there might be more illegal ones"
                );
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }

        if deletion_strategy == ContinuationStrategy::AllRequeue {
            Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout))
        } else {
            Ok(ReconcileFunctionAction::Continue)
        }
    }

    /// This method can be used to find Pods that do not match a set of Nodes and required labels.
    ///
    /// All Pods must match at least one of the node list & required labels combinations.
    /// All that don't match will be returned.
    ///
    /// The idea is that you pass in a list of tuples, one tuple for each role group.
    /// Each tuple consists of a list of eligible nodes for that role group's LabelSelector and a
    /// Map of label keys to optional values.
    ///
    /// To clearly identify Pods (e.g. to distinguish two pods on the same node from each other) they
    /// usually need some labels (e.g. a `component` and a `role-group` label).     
    pub async fn delete_excess_pods(
        &self,
        nodes_and_labels: &[(Vec<Node>, LabelOptionalValueMap)],
        existing_pods: &[Pod],
        deletion_strategy: ContinuationStrategy,
    ) -> ReconcileResult<Error> {
        let excess_pods = find_excess_pods(nodes_and_labels, existing_pods);
        for excess_pod in excess_pods {
            info!(
                "Deleting excess Pod [{}]",
                pod_utils::get_log_name(excess_pod)
            );
            self.client.delete(excess_pod).await?;

            if deletion_strategy == ContinuationStrategy::OneRequeue {
                trace!(
                    "Will requeue after deleting an excess pod, there might be more illegal ones"
                );
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }

        if deletion_strategy == ContinuationStrategy::AllRequeue {
            Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout))
        } else {
            Ok(ReconcileFunctionAction::Continue)
        }
    }

    /// This reconcile function can be added to the chain to automatically handle deleted objects
    /// using finalizers.
    ///
    /// It'll add a finalizer to the object if it's not there yet, if the `deletion_timestamp` is set
    /// it'll call the provided handler function and it'll remove the finalizer if the handler completes
    /// with a `Done` result.
    ///
    /// If the object is not deleted this function will return a `Continue` event.
    ///
    /// # Arguments
    ///
    /// * `handler` - This future will be completed if the object has been marked for deletion
    /// * `finalizer` - The finalizer to add and/or check for
    /// * `requeue_if_changed` - If this is `true` we'll return a `Requeue` immediately if we had to
    ///     change the resource due to the addition of the finalizer
    pub async fn handle_deletion(
        &self,
        handler: Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Error>> + Send + '_>>,
        finalizer: &str,
        requeue_if_changed: bool,
    ) -> ReconcileResult<Error>
    where
        T: Clone + DeserializeOwned + Meta + Send + Sync + 'static,
    {
        let being_deleted = finalizer::has_deletion_stamp(&self.resource);

        // Try to add a finalizer but only if the deletion_timestamp is not already set
        // Kubernetes forbids setting new finalizers on objects under deletion and will return this error:
        // Forbidden: no new finalizers can be added if the object is being deleted, found new finalizers []string{\"foo\"}
        if !being_deleted
            && finalizer::add_finalizer(&self.client, &self.resource, finalizer).await?
            && requeue_if_changed
        {
            return Ok(self.requeue());
        }

        if !being_deleted {
            debug!("Resource not deleted, continuing",);
            return Ok(ReconcileFunctionAction::Continue);
        }

        if !finalizer::has_finalizer(&self.resource, finalizer) {
            debug!("Resource being deleted but our finalizer is already gone, there might be others but we're done here!");
            return Ok(ReconcileFunctionAction::Done);
        }

        match handler.await? {
            ReconcileFunctionAction::Continue => Ok(ReconcileFunctionAction::Continue),
            ReconcileFunctionAction::Done => {
                info!("Removing finalizer [{}]", finalizer,);
                finalizer::remove_finalizer(&self.client, &self.resource, finalizer).await?;
                Ok(ReconcileFunctionAction::Done)
            }
            ReconcileFunctionAction::Requeue(_) => Ok(self.requeue()),
        }
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

    /// A reconciler function to add to our finalizer to the list of finalizers.
    /// It is a wrapper around [`finalizer::add_finalizer`].
    ///
    /// It can return `Continue` or `Requeue` depending on the `requeue` argument and the state of the resource.
    /// If the finalizer already exists it'll _always_ return `Continue`.
    ///
    /// There is a more full-featured alternative to this function ([`handle_deletion`]).
    ///
    /// # Arguments
    ///
    /// * `finalizer` - The finalizer to add
    /// * `requeue` - If `true` this function will return `Requeue` if the object was changed (i.e. the finalizer was added) otherwise it'll return `Continue`
    pub async fn add_finalizer(&self, finalizer: &str, requeue: bool) -> ReconcileResult<Error> {
        if finalizer::add_finalizer(&self.client, &self.resource, finalizer).await? && requeue {
            Ok(self.requeue())
        } else {
            Ok(ReconcileFunctionAction::Continue)
        }
    }
}

/// This returns `false` for Pods that have no OwnerReference (with a Controller flag)
/// or where the Controller does not have the same `uid` as the passed in `owner_uid`.
/// If however the `uid` exists and matches we return `true`.
fn pod_owned_by(pod: &Pod, owner_uid: &str) -> bool {
    let controller = controller_ref::get_controller_of(pod);
    matches!(controller, Some(OwnerReference { uid, .. }) if uid == owner_uid)
}

fn wait_for_running_and_ready_pods(
    requeue_timeout: &Duration,
    pods: &[Pod],
) -> ReconcileResult<Error> {
    let not_ready = pods
        .iter()
        .filter(|pod| !pod_utils::is_pod_running_and_ready(pod))
        .collect::<Vec<_>>();

    if !not_ready.is_empty() {
        let pods = not_ready
            .iter()
            .map(|pod| pod_utils::get_log_name(*pod))
            .collect::<Vec<_>>();
        let pods = pods.join(", ");
        trace!("Waiting for Pods to become ready: [{}]", pods);
        return Ok(ReconcileFunctionAction::Requeue(*requeue_timeout));
    }

    Ok(ReconcileFunctionAction::Continue)
}

fn wait_for_terminating_pods(requeue_timeout: &Duration, pods: &[Pod]) -> ReconcileResult<Error> {
    match pods.iter().any(|pod| finalizer::has_deletion_stamp(pod)) {
        true => {
            info!("Found terminating pods, requeuing to await termination!");
            Ok(ReconcileFunctionAction::Requeue(*requeue_timeout))
        }
        false => {
            debug!("No terminating pods found, continuing");
            Ok(ReconcileFunctionAction::Continue)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::PodBuilder;
    use chrono::Utc;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

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

    #[test]
    fn test_wait_for_running_and_ready_pods() {
        let duration = Duration::from_secs(30);
        let action = ReconcileFunctionAction::Requeue(duration);

        let pod1 = PodBuilder::new().name("pod1").build();
        let pod2 = PodBuilder::new().name("pod2").build();
        let pods = vec![pod1, pod2];
        let result = wait_for_running_and_ready_pods(&duration, &pods).unwrap();
        assert_eq!(result, action);

        let result = wait_for_running_and_ready_pods(&duration, &vec![]).unwrap();
        assert_eq!(result, ReconcileFunctionAction::Continue);

        let pod1 = PodBuilder::new().name("pod1").phase("Running").build();
        let result = wait_for_running_and_ready_pods(&duration, vec![pod1].as_slice()).unwrap();
        assert_eq!(result, action);

        let pod1 = PodBuilder::new()
            .name("pod1")
            .phase("Running")
            .with_condition("Ready", "True")
            .build();
        let result =
            wait_for_running_and_ready_pods(&duration, vec![pod1.clone()].as_slice()).unwrap();
        assert_eq!(result, ReconcileFunctionAction::Continue);

        let pod2 = PodBuilder::new().name("pod2").build();
        let result =
            wait_for_running_and_ready_pods(&duration, vec![pod1, pod2].as_slice()).unwrap();
        assert_eq!(result, action);
    }

    #[test]
    fn test_wait_for_terminating_pods() {
        let duration = Duration::from_secs(30);
        let action = ReconcileFunctionAction::Requeue(duration);

        let pod1 = PodBuilder::new()
            .deletion_timestamp(Time(Utc::now()))
            .build();

        let result = wait_for_terminating_pods(&duration, vec![pod1.clone()].as_slice()).unwrap();
        assert_eq!(result, action);

        let pod2 = PodBuilder::new().build();
        let result = wait_for_terminating_pods(&duration, vec![pod2.clone()].as_slice()).unwrap();
        assert_eq!(result, ReconcileFunctionAction::Continue);

        let result = wait_for_terminating_pods(&duration, vec![pod1, pod2].as_slice()).unwrap();
        assert_eq!(result, action);
    }
}
