use crate::pod_utils;
use k8s_openapi::api::core::v1::{Node, Pod};
use std::collections::BTreeMap;

/// This type is used in places where we need label keys with optional values.
pub type LabelOptionalValueMap = BTreeMap<String, Option<String>>;

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
/// usually need some labels (e.g. a `role` label).
pub fn find_excess_pods<'a>(
    nodes_and_required_labels: &[(Vec<Node>, LabelOptionalValueMap)],
    existing_pods: &'a [Pod],
) -> Vec<&'a Pod> {
    let mut used_pods = Vec::new();

    // For each pair of Nodes and labels we try to find all Pods that are currently in use and valid
    // We collect all of those in one big list.
    for (eligible_nodes, mandatory_label_values) in nodes_and_required_labels {
        let mut found_pods =
            find_valid_pods_for_nodes(&eligible_nodes, &existing_pods, mandatory_label_values);
        used_pods.append(&mut found_pods);
    }

    // Here we'll filter all existing Pods and will remove all Pods that are in use
    existing_pods.iter()
        .filter(|pod| {
            !used_pods
                .iter()
                .any(|used_pod|
                    matches!((pod.metadata.uid.as_ref(), used_pod.metadata.uid.as_ref()), (Some(existing_uid), Some(used_uid)) if existing_uid == used_uid))
        })
        .collect()
}

/// This function can be used to get a list of valid Pods that are assigned
/// (via their `spec.node_name` property) to one of a list of candidate nodes.
///
/// This is useful to find all _valid_ pods (i.e. ones that are actually required by an Operator)
/// so it can be compared against _all_ Pods that belong to the Controller.
///
/// All Pods that are not actually in use can be deleted.
pub fn find_valid_pods_for_nodes<'a>(
    candidate_nodes: &[Node],
    existing_pods: &'a [Pod],
    required_labels: &LabelOptionalValueMap,
) -> Vec<&'a Pod> {
    existing_pods
        .iter()
        .filter(|pod|
            // This checks whether the Pod has all the required labels and if it does
            // it'll try to find a Node with the same `node_name` as the Pod.
            pod_utils::pod_matches_labels(pod, required_labels) && candidate_nodes.iter().any(|node| pod_utils::is_pod_assigned_to_node(pod, node))
        )
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
/// You would usually call this function once per role group.
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
/// In this scenario you'd add a label `app.kubernetes.io/component` with the value `NameNode` to each
/// NameNode Pod.
/// And this is the label you can now filter on using the `label_values` argument.
///
/// NOTE: This method currently does not support multiple instances per Node!
pub fn find_nodes_that_need_pods<'a>(
    candidate_nodes: &'a [Node],
    existing_pods: &[Pod],
    label_values: &BTreeMap<String, Option<String>>,
) -> Vec<&'a Node> {
    candidate_nodes
        .iter()
        .filter(|node| {
            !existing_pods.iter().any(|pod| {
                pod_utils::is_pod_assigned_to_node(pod, node)
                    && pod_utils::pod_matches_labels(pod, label_values)
            })
        })
        .collect::<Vec<&Node>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::test::{NodeBuilder, PodBuilder};

    #[test]
    fn test_find_excess_pods() {
        let node1 = NodeBuilder::new().name("node1").build();
        let node2 = NodeBuilder::new().name("node2").build();
        let node3 = NodeBuilder::new().name("node3").build();
        let node4 = NodeBuilder::new().name("node4").build();
        let node5 = NodeBuilder::new().name("node5").build();

        let mut labels1 = BTreeMap::new();
        labels1.insert("group1".to_string(), None);

        let mut labels2 = BTreeMap::new();
        labels2.insert("group2".to_string(), Some("foobar".to_string()));

        let nodes_and_labels = vec![
            (vec![node1, node2, node3.clone()], labels1),
            (vec![node3, node4, node5], labels2),
        ];

        let pod = PodBuilder::new().node_name("node1").build();
        let pods = vec![pod];

        let excess_pods = find_excess_pods(nodes_and_labels.as_slice(), &pods);
        assert_eq!(excess_pods.len(), 1);
    }

    #[test]
    fn test_find_valid_pods_for_nodes() {
        // Two nodes, one pod, no labels on pod, but looking for labels, shouldn't match
        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![PodBuilder::new().node_name("foobar").build()];

        let mut label_values = BTreeMap::new();
        label_values.insert("foo".to_string(), Some("bar".to_string()));

        assert_eq!(
            0,
            find_valid_pods_for_nodes(&nodes, &existing_pods, &label_values).len()
        );

        // Two nodes, one pod, matching labels on pod, but looking for labels, should match
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "bar".to_string());

        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![PodBuilder::new()
            .node_name("foobar")
            .with_labels(pod_labels)
            .build()];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            1,
            find_valid_pods_for_nodes(&nodes, &existing_pods, &expected_labels).len()
        );

        // Two nodes, one pod, matching label key on pod but wrong value, but looking for labels, shouldn't match
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "WRONG".to_string());

        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![PodBuilder::new()
            .node_name("foobar")
            .with_labels(pod_labels)
            .build()];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            0,
            find_valid_pods_for_nodes(&nodes, &existing_pods, &expected_labels).len()
        );

        // Two nodes, two pods. one matches the other doesn't
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("foo".to_string(), "bar".to_string());

        let nodes = vec![
            test::build_test_node("foobar"),
            test::build_test_node("barfoo"),
        ];
        let existing_pods = vec![
            PodBuilder::new()
                .node_name("foobar")
                .with_labels(pod_labels.clone())
                .build(),
            PodBuilder::new()
                .node_name("wrong_node")
                .with_labels(pod_labels)
                .build(),
        ];

        let mut expected_labels = BTreeMap::new();
        expected_labels.insert("foo".to_string(), Some("bar".to_string()));
        assert_eq!(
            1,
            find_valid_pods_for_nodes(&nodes, &existing_pods, &expected_labels).len()
        );
    }

    #[test]
    fn test_find_nodes_that_need_pods() {
        let foo_node = NodeBuilder::new().name("foo").build();
        let foo_pod = PodBuilder::new().node_name("foo").build();

        let mut labels = BTreeMap::new();
        labels.insert("foo".to_string(), Some("bar".to_string()));

        let nodes = vec![foo_node];
        let pods = vec![foo_pod];

        let need_pods = find_nodes_that_need_pods(nodes.as_slice(), pods.as_slice(), &labels);
        assert_eq!(need_pods.len(), 1);

        let foo_pod = PodBuilder::new()
            .node_name("foo")
            .with_label("foo", "bar")
            .build();
        let pods = vec![foo_pod];
        let need_pods = find_nodes_that_need_pods(nodes.as_slice(), pods.as_slice(), &labels);
        assert!(need_pods.is_empty());

        labels.clear();
        let need_pods = find_nodes_that_need_pods(nodes.as_slice(), pods.as_slice(), &labels);
        assert!(need_pods.is_empty());
    }
}
