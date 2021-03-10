use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::{conditions, controller_ref, finalizer, podutils};

use crate::conditions::ConditionStatus;
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector, OwnerReference};
use kube::api::{ListParams, Meta, ObjectMeta};
use kube_runtime::controller::ReconcilerAction;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, HashMap};
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

    fn requeue(&self) -> ReconcileFunctionAction {
        ReconcileFunctionAction::Requeue(self.requeue_timeout)
    }

    pub async fn wait_for_running_and_ready_pods(&self, pods: &[Pod]) -> ReconcileResult<Error> {
        for pod in pods {
            if !podutils::is_pod_running_and_ready(pod) {
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }
        Ok(ReconcileFunctionAction::Continue)
    }

    // TODO: Docs & Test
    pub async fn wait_for_terminating_pods(&self, pods: &[Pod]) -> ReconcileResult<Error> {
        match pods.iter().any(|pod| finalizer::has_deletion_stamp(pod)) {
            true => {
                info!("Found terminating pods, requeuing to await termination!");
                Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout))
            }
            false => {
                debug!("No terminating pods found, continuing");
                Ok(ReconcileFunctionAction::Continue)
            }
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

    /// This lists all Pods that have an OwnerReference that points to us (the object from `self.resource`)
    /// as its Controller.
    ///
    /// Unfortunately the Kubernetes API does _not_ allow filtering by OwnerReference so we have to fetch
    /// all Pods and filter them on the client.
    /// To avoid this overhead provide a LabelSelector to narrow down the candidates.
    /// TODO: LabelSelector not possible yet
    pub async fn list_pods(&self) -> OperatorResult<Vec<Pod>> {
        let api = self.client.get_namespaced_api(&self.namespace());

        // TODO: In addition to filtering by OwnerReference (which can only be done client-side)
        // we could also add a custom label.
        // TODO: This can use the new list_with_label_selector method from Client

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

    /// Finds nodes in the cluster that match a given LabelSelector
    /// This takes a hashmap of String -> LabelSelector and returns
    /// a map with found nodes per String
    ///
    /// This will only match Stackable nodes (Nodes with a special label).
    /// TODO: Docs & Tests
    pub async fn find_nodes_that_fit_selectors(
        &self,
        roles: &HashMap<String, LabelSelector>,
    ) -> OperatorResult<HashMap<String, Vec<Node>>> {
        let mut found_nodes = HashMap::new();
        for (group_name, selector) in roles {
            let selector = add_stackable_selector(selector);
            let nodes = self.client.list_with_label_selector(&selector).await?;
            debug!(
                "Found [{}] nodes for role group [{}]: [{:?}]",
                nodes.len(),
                group_name,
                nodes
            );
            found_nodes.insert(group_name.clone(), nodes);
        }
        Ok(found_nodes)
    }

    /// Checks all passed Pods to see if they fulfil some basic requirements.
    ///
    /// * They need to have all required labels and optionally one of a list of allowed values
    /// * They need to have a spec.node_name
    ///
    /// If not they are considered invalid and will be deleted.

    // TODO: delete_illegal_and_excess_pods and maybe take a list of BTreeMap<String<Option<String>> and convert it to the
    // structure we need so users need to build the labels only once
    pub async fn delete_illegal_pods<'a>(
        &self,
        pods: &'a [Pod],
        required_labels: &BTreeMap<String, Option<Vec<String>>>,
        deletion_strategy: DeletionStrategy,
    ) -> ReconcileResult<Error> {
        let illegal_pods = podutils::find_invalid_pods(pods, required_labels);
        if illegal_pods.is_empty() {
            return Ok(ReconcileFunctionAction::Continue);
        }

        for illegal_pod in illegal_pods {
            info!(
                "Deleting invalid Pod [{}]",
                podutils::get_log_name(illegal_pod)
            );
            self.client.delete(illegal_pod).await?;

            if deletion_strategy == DeletionStrategy::OneRequeue {
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }

        if deletion_strategy == DeletionStrategy::AllRequeue {
            return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
        }

        Ok(ReconcileFunctionAction::Continue)
    }

    pub async fn delete_excess_pods(
        &self,
        nodes_and_labels: &[(Vec<Node>, BTreeMap<String, Option<String>>)],
        existing_pods: &[Pod],
        deletion_strategy: DeletionStrategy,
    ) -> ReconcileResult<Error> {
        let excess_pods = podutils::find_excess_pods(nodes_and_labels, existing_pods);
        for excess_pod in excess_pods {
            info!(
                "Deleting invalid Pod [{}]",
                podutils::get_log_name(excess_pod)
            );
            self.client.delete(excess_pod).await?;

            if deletion_strategy == DeletionStrategy::OneRequeue {
                return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
            }
        }

        if deletion_strategy == DeletionStrategy::AllRequeue {
            return Ok(ReconcileFunctionAction::Requeue(self.requeue_timeout));
        }

        Ok(ReconcileFunctionAction::Continue)
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

/// Helper method to make sure that any LabelSelector we use only matches our own "special" nodes.
/// At the moment this label is "type" with the value "krustlet" and we'll use match_labels.
///
/// WARN: Should a label "type" already be used this will be overridden!
/// If this is really needed add a match…expression
///
/// We will not however change the original LabelSelector, a new one will be returned.
fn add_stackable_selector(selector: &LabelSelector) -> LabelSelector {
    let mut selector = selector.clone();
    selector
        .match_labels
        .get_or_insert_with(BTreeMap::new)
        .insert("type".to_string(), "krustlet".to_string());
    selector
}

/// This function can be used to find Nodes that are missing Pods.
///
/// It uses a simple label selector to find matching nodes.
/// This is not a full LabelSelector because the expectation is that the calling code used a
/// full LabelSelector to query the Kubernetes API for a set of candidate Nodes.
///
/// We now need to check whether these candidate nodes already contain a Pod or not.
/// That's why we also pass in _all_ Pods that we know about and one or more labels (including optional values).
/// This method checks if there are pods assigned to a node and if these pods have all required labels.
/// These labels are _not_ meant to be user-defined but can be used to distinguish between different Pod types.
///
/// # Example
///
/// * HDFS has multiple roles (NameNode, DataNode, JournalNode)
/// * Multiple roles may run on the same node
///
/// To check whether a certain Node is already running a NameNode Pod it is not enough to just check
/// if there is a Pod assigned to that node.
/// We also need to be able to distinguish the different roles.
/// That's where the labels come in.
/// In this scenario you'd add a label `hdfs.stackable.tech/role` with the value `NameNode` to each
/// NameNode Pod.
/// And this is the label you can now filter on using the `label_values` argument.

// TODO: Tests
pub async fn find_nodes_that_need_pods<'a>(
    candidate_nodes: &'a [Node],
    existing_pods: &[Pod],
    label_values: &BTreeMap<String, Option<String>>,
) -> Vec<&'a Node> {
    candidate_nodes
        .iter()
        .filter(|node| {
            !existing_pods.iter().any(|pod| {
                podutils::is_pod_assigned_to_node(pod, node)
                    && podutils::pod_matches_labels(pod, &label_values)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

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
    fn test_add_stackable_selector() {
        let mut ls = LabelSelector {
            match_expressions: None,
            match_labels: None,
        };

        // LS didn't have any match_label
        assert!(
            matches!(add_stackable_selector(&ls).match_labels, Some(labels) if labels.get("type").unwrap() == "krustlet")
        );

        // LS has labels but no conflicts with our own
        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), "bar".to_string());

        ls.match_labels = Some(labels);
        assert!(
            matches!(add_stackable_selector(&mut ls).match_labels, Some(labels) if labels.get("type").unwrap() == "krustlet")
        );

        // LS already has a LS that matches our internal one
        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), "bar".to_string());
        labels.insert("type".to_string(), "foobar".to_string());
        ls.match_labels = Some(labels);
        assert!(
            matches!(add_stackable_selector(&mut ls).match_labels, Some(labels) if labels.get("type").unwrap() == "krustlet")
        );
    }
}
