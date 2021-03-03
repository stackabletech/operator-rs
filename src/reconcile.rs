use crate::client::Client;
use crate::error::{Error, OperatorResult};
use crate::{conditions, controller_ref, podutils};

use crate::conditions::ConditionStatus;
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, OwnerReference};
use kube::api::{ListParams, Meta, ObjectMeta};
use kube_runtime::controller::ReconcilerAction;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
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

    /*
    /// Finds nodes in the cluster that match a given labelselector
    /// This takes a hashmap of String -> LabelSelector and returns
    /// a map with found nodes per String
    pub async fn find_nodes_that_fit_selectors(
        &self,
        node_groups: HashMap<String, LabelSelector>,
    ) -> Result<HashMap<String, Vec<Node>>, Error> {
        let mut found_nodes = HashMap::new();
        for (group_name, selector) in node_groups {
            let nodes = self.get_nodes_for_selector(&selector).await?;
            debug!(
                "Found [{}] nodes for datanode group [{}]: [{:?}]",
                nodes.len(),
                group_name,
                nodes
            );
            found_nodes.insert(group_name, nodes);
        }
        Ok(found_nodes)
    }
     */
    /*
        async fn get_nodes_for_selector(&self, selector: &LabelSelector) -> Result<Vec<Node>, Error> {
            let api = self.client.get_api(None);
            let selector_with_stackable = add_stackable_selector(selector);
            let selector_string = convert_label_selector_to_query_string(selector_with_stackable);
            debug!(
                "Got labelselector: [{}]",
                selector_string.as_ref().unwrap_or(&String::from("None"))
            );
            let list_params = ListParams {
                label_selector: Some(selector_string),
                ..ListParams::default()
            };
            api.list(&list_params)
                .await
                .map_err(Error::from)
                .map(|result| result.items)
                .map(|nodes| nodes.into_iter().collect())
        }
    */
    /*
    /// Retrieves all pods that are owned by the resource and removes those that do not have all
    /// mandatory labels
    pub async fn delete_illegal_pods(&self, required_labels: &[&str]) -> OperatorResult<Vec<Pod>> {
        let existing_pods = self.list_pods().await?;
        let mut deleted_pods = vec![];
        for pod in existing_pods {
            if let (
                Some(labels),
                Some(PodSpec {
                    node_name: Some(node_name),
                    ..
                }),
            ) = (&pod.metadata.labels, &pod.spec)
            {
                for label_name in required_labels {
                    if labels.get(label_name as &str).is_none() {
                        error!("Pod [{:?}] does not have the `{}` label, this is illegal, deleting it.",
                                pod, label_name);
                        deleted_pods.push(pod.clone());
                        self.client.delete(&pod).await?;
                        continue;
                    }
                }
            } else {
                error!("HdfsCluster {}: Pod [{:?}] does not have any spec or labels, this is illegal, deleting it.",
                        self.log_name(),
                        pod);
                deleted_pods.push(pod.clone());
                self.client.delete(&pod).await?;
            }
        }
        Ok(deleted_pods)
    }

     */

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
}

/// This returns `false` for Pods that have no OwnerReference (with a Controller flag)
/// or where the Controller does not have the same `uid` as the passed in `owner_uid`.
/// If however the `uid` exists and matches we return `true`.
fn pod_owned_by(pod: &Pod, owner_uid: &str) -> bool {
    let controller = controller_ref::get_controller_of(pod);
    matches!(controller, Some(OwnerReference { uid, .. }) if uid == owner_uid)
}

/*
fn add_stackable_selector(selector: &LabelSelector) -> LabelSelector {
    let mut selector = selector.clone();
    // Add our specific Stackable selector to ensure we target only Stackable nodes
    if selector.match_labels.is_none() {
        selector.match_labels = Some(BTreeMap::new());
    }
    selector
        .match_labels
        .unwrap()
        .insert("type".to_string(), "krustlet".to_string());
    selector
}
*/
/*
pub async fn find_excess_pods(
    things: Vec<(Vec<Node>, HashMap<String, Option<String>>)>,
    pods: Vec<Pod>,
) -> Vec<Pod> {
    let mut pods_in_use = Vec::new();
    for (eligible_nodes, mandatory_label_values) in things {
        let mut found_pods =
            find_pods_that_are_in_use(&eligible_nodes, &pods, mandatory_label_values);
        pods_in_use.append(&mut found_pods);
    }

    pods.into_iter()
        .filter(|pod| {
            !pods_in_use
                .iter()
                .any(|used_pod| pod.metadata.uid.unwrap() == used_pod.metadata.uid.unwrap())
        })
        .collect()
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
                    && pod_matches_labels(pod, &label_values)
            })
        })
        .collect()
}

/// This function can be used to get a list of Pods that are assigned (via their `spec.node_name` property)
/// to specific nodes.
///
/// This is useful to find all _valid_ pods (i.e. ones that are actually required by an Operator)
/// so it can be compared against _all_ Pods that belong to the Controller.
/// All Pods that are not actually in use can be deleted.
pub fn find_pods_that_are_in_use<'a>(
    candidate_nodes: &[Node],
    existing_pods: &'a [Pod],
    label_values: &BTreeMap<String, Option<String>>,
) -> Vec<&'a Pod> {
    existing_pods
        .iter()
        .filter(|pod|
            // This checks whether the Pod has all the required labels and if it does
            // it'll try to find a Node with the same `node_name` as the Pod.
            pod_matches_labels(pod, &label_values) && candidate_nodes.iter().any(|node| podutils::is_pod_assigned_to_node(pod, node))
        )
        .collect()
}

fn pod_matches_labels(pod: &Pod, expected_labels: &BTreeMap<String, Option<String>>) -> bool {
    let pod_labels = &pod.metadata.labels;

    for (expected_key, expected_value) in expected_labels {
        // We only do this here because `expected_labels` might be empty in which case
        // it's totally fine if the Pod doesn't have any labels.
        // Now however we're definitely looking for a key so if the Pod doesn't have any labels
        // it will never be able to match.
        let pod_labels = match pod_labels {
            None => return false,
            Some(pod_labels) => pod_labels,
        };

        // We can match two kinds:
        //   * Just the label key (expected_value == None)
        //   * Key and Value
        if !pod_labels.contains_key(expected_key) {
            return false;
        }

        if let Some(expected_value) = expected_value {
            // unwrap is fine here as we already checked earlier if the key exists
            if pod_labels.get(expected_key).unwrap() != expected_value {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::PodSpec;
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
    fn test_pod_matches_labels() {
        let mut test_labels = BTreeMap::new();
        test_labels.insert("label1".to_string(), "test1".to_string());
        test_labels.insert("label2".to_string(), "test2".to_string());
        test_labels.insert("label3".to_string(), "test3".to_string());

        let test_pod = build_test_pod(None, Some(test_labels));

        // Pod matches a label, should match
        let mut matching_labels1 = BTreeMap::new();
        matching_labels1.insert(String::from("label1"), Some(String::from("test1")));
        assert!(pod_matches_labels(&test_pod, &matching_labels1));

        // Pod matches a label, should match
        let mut matching_labels2 = BTreeMap::new();
        matching_labels2.insert(String::from("label2"), Some(String::from("test2")));
        assert!(pod_matches_labels(&test_pod, &matching_labels2));

        // Pods that are missing a label should not match
        let mut non_matching_labels1 = BTreeMap::new();
        non_matching_labels1.insert(String::from("wrong_label"), Some(String::from("test2")));
        assert!(!pod_matches_labels(&test_pod, &non_matching_labels1));

        // Empty list should match all pods - we have no requirements that the pod
        // has to meet
        let empty_labels = BTreeMap::new();
        assert!(pod_matches_labels(&test_pod, &empty_labels));

        // Pod matches only one of two required labels, should not match
        let mut non_matching_multiple_labels = BTreeMap::new();
        non_matching_multiple_labels.insert(String::from("label1"), Some(String::from("test1")));
        non_matching_multiple_labels.insert(String::from("label2"), Some(String::from("test1")));
        assert!(!pod_matches_labels(
            &test_pod,
            &non_matching_multiple_labels
        ));

        // Pod matches both labels, should match
        let mut matching_multiple_labels = BTreeMap::new();
        matching_multiple_labels.insert(String::from("label1"), Some(String::from("test1")));
        matching_multiple_labels.insert(String::from("label2"), Some(String::from("test2")));
        assert!(pod_matches_labels(&test_pod, &matching_multiple_labels));

        // Pod has required label without specified value, should match
        let mut matching_label_present = BTreeMap::new();
        matching_label_present.insert(String::from("label1"), None);
        assert!(pod_matches_labels(&test_pod, &matching_label_present));

        // Pod has multiple required labels without specified value, should match
        let mut matching_multiple_label_present = BTreeMap::new();
        matching_multiple_label_present.insert(String::from("label1"), None);
        matching_multiple_label_present.insert(String::from("label3"), None);
        assert!(pod_matches_labels(
            &test_pod,
            &matching_multiple_label_present
        ));

        // Pod has one label missing and one present - should not match
        let mut matching_label_present_and_missing = BTreeMap::new();
        matching_label_present_and_missing.insert(String::from("label1"), None);
        matching_label_present_and_missing.insert(String::from("label4"), None);
        assert!(!pod_matches_labels(
            &test_pod,
            &matching_label_present_and_missing
        ));

        // Pod has _no_ labels, should not match because we are definitely asking for labels
        let test_pod = build_test_pod(None, None);
        let mut matching_label_present_and_missing = BTreeMap::new();
        matching_label_present_and_missing.insert(String::from("label1"), None);
        matching_label_present_and_missing.insert(String::from("label4"), None);
        assert!(!pod_matches_labels(
            &test_pod,
            &matching_label_present_and_missing
        ));

        // Pod has _no_ labels but we're also asking for no labels
        assert!(pod_matches_labels(&test_pod, &BTreeMap::new()));
    }

    #[test]
    fn test_find_pods_that_are_in_use() {
        // Two nodes, one pod, no labels on pod, but looking for labels, shouldn't match
        let nodes = vec![build_test_node("foobar"), build_test_node("barfoo")];
        let existing_pods = vec![build_test_pod(Some("foobar"), None)];

        let mut label_values = BTreeMap::new();
        label_values.insert("foo".to_string(), Some("bar".to_string()));

        assert_eq!(
            0,
            find_pods_that_are_in_use(&nodes, &existing_pods, &label_values).len()
        );

        // Two nodes, one pod, matching labels on pod, but looking for labels, should match
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "bar".to_string());

        let nodes = vec![build_test_node("foobar"), build_test_node("barfoo")];
        let existing_pods = vec![build_test_pod(Some("foobar"), Some(pod_labels))];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            1,
            find_pods_that_are_in_use(&nodes, &existing_pods, &expected_labels).len()
        );

        // Two nodes, one pod, matching label key on pod but wrong value, but looking for labels, shouldn't match
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "WRONG".to_string());

        let nodes = vec![build_test_node("foobar"), build_test_node("barfoo")];
        let existing_pods = vec![build_test_pod(Some("foobar"), Some(pod_labels))];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            0,
            find_pods_that_are_in_use(&nodes, &existing_pods, &expected_labels).len()
        );

        // Two nodes, two pods. one matches the other doesn't
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "bar".to_string());

        let nodes = vec![build_test_node("foobar"), build_test_node("barfoo")];
        let existing_pods = vec![
            build_test_pod(Some("foobar"), Some(pod_labels.clone())),
            build_test_pod(Some("wrong_node"), Some(pod_labels.clone())),
        ];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            1,
            find_pods_that_are_in_use(&nodes, &existing_pods, &expected_labels).len()
        );
    }

    fn build_test_node(name: &str) -> Node {
        Node {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..ObjectMeta::default()
            },
            spec: None,
            status: None,
        }
    }

    fn build_test_pod(node_name: Option<&str>, labels: Option<BTreeMap<String, String>>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                labels,
                ..ObjectMeta::default()
            },
            spec: Some(PodSpec {
                node_name: node_name.map(|name| name.to_string()),
                ..PodSpec::default()
            }),
            status: None,
        }
    }
}
