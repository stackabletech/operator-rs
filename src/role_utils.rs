//! This module provides utility functions for dealing with role (types) and role groups.
//!
//! While other modules in this crate try to be generic and reusable for other operators
//! this one makes very specific assumptions about how a CRD is structured.
//!
//! These assumptions are detailed and explained below.
//!
//! # Roles / Role types
//!
//! A CRD is often used to operate another piece of software.
//! Software - especially the distributed kind - sometimes consists of multiple different types of program working together to achieve their goal.
//! These different types are what we call a _role_.
//!
//! ## Examples
//!
//! Apache Hadoop HDFS:
//! * NameNode
//! * DataNode
//! * JournalNode
//!
//! Kubernetes:
//! * kube-apiserver
//! * kubelet
//! * kube-controller-manager
//! * ...
//!
//! # Role Groups
//!
//! There is sometimes a need to have different configuration options or different label selectors for different instances of the same role.
//! Role groups are what allows this.
//! Nested under a role there can be multiple role groups, each with its own LabelSelector and configuration.
//!
//! ## Example
//!
//! Role Groups: `default`, `20-cores`, `gpu`
//!
//! # Pod labels
//!
//! Each Pod that Operators create needs to have a common set of labels:
//! * <TODO>/name - The name of the parent resource, this is useful so an operator can list all its pods by using a LabelSelector
//! * <TODO>/role - The role/role type, this is used to distinguish multiple pods on the same node from each other
//! * <TODO>roleGroup - The name of the role group this pod belongs to
//!
//! Each Pod can have more operator specific labels.

use crate::podutils;
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use std::collections::BTreeMap;

pub struct RoleGroup {
    pub name: String,
    pub selector: LabelSelector,
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
/// usually need some labels (e.g. a `role` label).
pub fn find_excess_pods<'a>(
    nodes_and_required_labels: &[(Vec<Node>, BTreeMap<String, Option<String>>)],
    existing_pods: &'a [Pod],
) -> Vec<&'a Pod> {
    let mut used_pods = Vec::new();

    // For each pair of Nodes and labels we try to find all Pods that are currently in use and valid
    // We collect all of those in one big list.
    for (eligible_nodes, mandatory_label_values) in nodes_and_required_labels {
        let mut found_pods = podutils::find_valid_pods_for_nodes(
            &eligible_nodes,
            &existing_pods,
            mandatory_label_values,
        );
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

#[cfg(test)]
mod tests {

    use crate::role_utils::find_excess_pods;
    use crate::test::{NodeBuilder, PodBuilder};
    use std::collections::BTreeMap;

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
}
