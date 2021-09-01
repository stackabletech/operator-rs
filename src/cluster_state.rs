use crate::role_utils::EligibleNodesForRoleAndGroup;
use rand::prelude::IteratorRandom;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::iter::FromIterator;

pub type RoleAndGroupNodeInfo<T> = BTreeMap<String, BTreeMap<String, NodeInfo<T>>>;

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo<T> {
    pub used_nodes: HashSet<String>,
    pub additional_info: Option<T>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterState<T> {
    node_info: RoleAndGroupNodeInfo<T>,
}

impl<T> ClusterState<T> {
    pub fn new(eligible_nodes: EligibleNodesForRoleAndGroup) -> Self {
        let mut node_info = BTreeMap::new();
        for (role_name, nodes_for_role_group) in eligible_nodes {
            let mut group_info = BTreeMap::new();
            for (group_name, eligible_nodes) in nodes_for_role_group {
                // randomly pick nodes depending on replicas or all nodes
                let used_nodes = match eligible_nodes.replicas {
                    Some(rep) => HashSet::from_iter(
                        eligible_nodes
                            .nodes
                            .into_iter()
                            .filter_map(|node| node.metadata.name)
                            .choose_multiple(&mut rand::thread_rng(), usize::from(rep)),
                    ),
                    None => HashSet::from_iter(
                        eligible_nodes
                            .nodes
                            .into_iter()
                            .filter_map(|node| node.metadata.name),
                    ),
                };

                group_info.insert(
                    group_name,
                    NodeInfo {
                        used_nodes,
                        additional_info: None,
                    },
                );
            }

            node_info.insert(role_name, group_info);
        }

        ClusterState { node_info }
    }

    pub fn collect_used_nodes(&self) -> HashSet<String> {
        let mut used_nodes = HashSet::new();
        for group in self.node_info.values() {
            for info in group.values() {
                used_nodes.extend(info.used_nodes.clone())
            }
        }

        used_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    #[derive(Debug)]
    pub struct AdditionalInfo {}

    #[rstest]
    #[case(
        r#"
        role_1:
          group_1:
            nodes:
              - metadata: 
                  name: node_1
              - metadata: 
                  name: node_2
              - metadata: 
                  name: node_3
            replicas: 1
          group_2:
            nodes:
              - metadata: 
                  name: node_3
              - metadata: 
                  name: node_4
        role_2:
          group_3:
            nodes:
              - metadata:
                  name: node_5
              - metadata:
                  name: node_6
            replicas: 2
          group_4:
            nodes:
              - metadata:
                  name: node_1
              - metadata:
                  name: node_2
            replicas: 10
    "#
    )]
    fn test_cluster_state(#[case] eligible_node_names: &str) {
        let eligible_nodes: EligibleNodesForRoleAndGroup =
            serde_yaml::from_str(eligible_node_names).expect("Invalid test definition!");

        let cluster_state: ClusterState<AdditionalInfo> = ClusterState::new(eligible_nodes);

        let used_nodes_role_1_group_1 = cluster_state
            .node_info
            .get("role_1")
            .and_then(|group| group.get("group_1").map(|info| &info.used_nodes))
            .unwrap();

        assert!(
            used_nodes_role_1_group_1.len() == 1
                && (used_nodes_role_1_group_1.contains("node_1")
                    || used_nodes_role_1_group_1.contains("node_2")
                    || used_nodes_role_1_group_1.contains("node_3"))
        );

        let used_nodes_role_1_group_2 = cluster_state
            .node_info
            .get("role_1")
            .and_then(|group| group.get("group_2").map(|info| &info.used_nodes))
            .unwrap();

        assert!(
            used_nodes_role_1_group_2.contains("node_3")
                && used_nodes_role_1_group_2.contains("node_4")
        );

        assert_eq!(cluster_state.collect_used_nodes().len(), 6);
        assert!(cluster_state.collect_used_nodes().contains("node_1"));
        assert!(cluster_state.collect_used_nodes().contains("node_2"));
        assert!(cluster_state.collect_used_nodes().contains("node_3"));
        assert!(cluster_state.collect_used_nodes().contains("node_4"));
        assert!(cluster_state.collect_used_nodes().contains("node_5"));
        assert!(cluster_state.collect_used_nodes().contains("node_6"));
    }
}
