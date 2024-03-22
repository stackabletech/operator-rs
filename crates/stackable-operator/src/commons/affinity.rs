use std::collections::BTreeMap;

use k8s_openapi::{
    api::core::v1::{
        NodeAffinity, PodAffinity, PodAffinityTerm, PodAntiAffinity, WeightedPodAffinityTerm,
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use schemars::{schema::Schema, JsonSchema};
use serde::{Deserialize, Serialize};
use stackable_operator_derive::Fragment;

use crate::{
    config::merge::{Atomic, Merge},
    kvp::consts::{K8S_APP_COMPONENT_KEY, K8S_APP_INSTANCE_KEY, K8S_APP_NAME_KEY},
};

pub const TOPOLOGY_KEY_HOSTNAME: &str = "kubernetes.io/hostname";

/// These configuration settings control
/// [Pod placement](DOCS_BASE_URL_PLACEHOLDER/concepts/operations/pod_placement).
#[derive(Clone, Debug, Default, Deserialize, Fragment, JsonSchema, PartialEq, Serialize)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct StackableAffinity {
    pub pod_affinity: Option<PodAffinity>,
    pub pod_anti_affinity: Option<PodAntiAffinity>,
    pub node_affinity: Option<NodeAffinity>,
    #[schemars(schema_with = "optional_stackable_node_selector_schema")]
    #[fragment_attrs(schemars(schema_with = "optional_stackable_node_selector_schema"))]
    pub node_selector: Option<StackableNodeSelector>,
}

/// We can not simply use [`BTreeMap<String, String>`] in [`StackableAffinity`], as the fields needs to be [`Atomic`].
/// We can not mark it as [`Atomic`], as [`crate::config::fragment::FromFragment`] is already implemented for
/// [`BTreeMap<String, String>`].
///
/// We `#[serde(flatten)]` the contained [`BTreeMap<String, String>`], so `serde_yaml` can deserialize everything as
/// expected. However the generated JsonSchema will be wrong, so we need to provide our custom one (see
/// <https://github.com/stackabletech/issues/issues/554> for details).
#[derive(Clone, Debug, Eq, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackableNodeSelector {
    #[serde(flatten)]
    pub node_selector: BTreeMap<String, String>,
}

impl Atomic for StackableNodeSelector {}

/// We need a custom JsonSchema for [`StackableNodeSelector`], please have a look at the documentation there.
pub fn optional_stackable_node_selector_schema(gen: &mut schemars::gen::SchemaGenerator) -> Schema {
    Option::<BTreeMap<String, String>>::json_schema(gen)
}

/// Creates a `WeightedPodAffinityTerm`, which expresses a affinity towards all Pods of the given product (`app_name`) instance (`cluster_name`) role (`role`).
/// This affinity can be used to attract towards (affinity) or away (anti-affinity) from the specified role.
/// One common example would be to use this to distribute all the Pods of a certain role, e.g. hdfs datanodes.
/// An other usage would be to attract the hbase regionservers towards hdfs datanodes.
pub fn affinity_between_role_pods(
    app_name: &str,
    cluster_name: &str,
    role: &str,
    weight: i32,
) -> WeightedPodAffinityTerm {
    WeightedPodAffinityTerm {
        pod_affinity_term: PodAffinityTerm {
            label_selector: Some(LabelSelector {
                match_expressions: None,
                match_labels: Some(BTreeMap::from([
                    (K8S_APP_NAME_KEY.to_string(), app_name.to_string()),
                    (K8S_APP_INSTANCE_KEY.to_string(), cluster_name.to_string()),
                    (K8S_APP_COMPONENT_KEY.to_string(), role.to_string()),
                    // We don't include the role-group label here, as the affinity should be between all rolegroups of the given role
                ])),
            }),
            namespace_selector: None,
            namespaces: None,
            topology_key: TOPOLOGY_KEY_HOSTNAME.to_string(),
        },
        weight,
    }
}

/// Creates a `WeightedPodAffinityTerm`, which expresses a affinity towards all Pods of the given product (`app_name`) instance (`cluster_name`).
/// This affinity can be used to attract towards (affinity) or away (anti-affinity) from the specified cluster.
/// One common example would be to use this to co-locate all the Pods of a certain cluster to not have to many network trips.
pub fn affinity_between_cluster_pods(
    app_name: &str,
    cluster_name: &str,
    weight: i32,
) -> WeightedPodAffinityTerm {
    WeightedPodAffinityTerm {
        pod_affinity_term: PodAffinityTerm {
            label_selector: Some(LabelSelector {
                match_expressions: None,
                match_labels: Some(BTreeMap::from([
                    (K8S_APP_NAME_KEY.to_string(), app_name.to_string()),
                    (K8S_APP_INSTANCE_KEY.to_string(), cluster_name.to_string()),
                ])),
            }),
            namespace_selector: None,
            namespaces: None,
            topology_key: TOPOLOGY_KEY_HOSTNAME.to_string(),
        },
        weight,
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::{
        api::core::v1::{NodeSelector, NodeSelectorRequirement, NodeSelectorTerm},
        apimachinery::pkg::apis::meta::v1::LabelSelectorRequirement,
    };

    use crate::config::fragment;

    use super::*;

    #[test]
    fn test_affinity_merge_new_attributes() {
        let default_affinity = StackableAffinityFragment {
            pod_affinity: None,
            pod_anti_affinity: Some(PodAntiAffinity {
                preferred_during_scheduling_ignored_during_execution: Some(vec![
                    affinity_between_role_pods("kafka", "simple-kafka", "broker", 70),
                ]),
                required_during_scheduling_ignored_during_execution: None,
            }),
            node_affinity: None,
            node_selector: None,
        };

        let role_input = r#"
          podAffinity:
            requiredDuringSchedulingIgnoredDuringExecution:
              - labelSelector:
                  matchExpressions:
                    - key: app.kubernetes.io/name
                      operator: In
                      values:
                        - foo
                        - bar
          nodeAffinity:
            requiredDuringSchedulingIgnoredDuringExecution:
              nodeSelectorTerms:
                - matchExpressions:
                  - key: topology.kubernetes.io/zone
                    operator: In
                    values:
                      - antarctica-east1
                      - antarctica-west1
          nodeSelector:
            disktype: ssd
          "#;
        let mut role_affinity: StackableAffinityFragment =
            serde_yaml::from_str(role_input).expect("illegal test input");

        role_affinity.merge(&default_affinity);
        let merged_affinity: StackableAffinity = fragment::validate(role_affinity).unwrap();

        assert_eq!(
            merged_affinity,
            StackableAffinity {
                pod_affinity: Some(PodAffinity {
                    preferred_during_scheduling_ignored_during_execution: None,
                    required_during_scheduling_ignored_during_execution: Some(vec![
                        PodAffinityTerm {
                            label_selector: Some(LabelSelector {
                                match_expressions: Some(vec![LabelSelectorRequirement {
                                    key: "app.kubernetes.io/name".to_string(),
                                    operator: "In".to_string(),
                                    values: Some(vec!["foo".to_string(), "bar".to_string()])
                                }]),
                                match_labels: None,
                            }),
                            namespace_selector: None,
                            namespaces: None,
                            topology_key: "".to_string(),
                        }
                    ])
                }),
                pod_anti_affinity: Some(PodAntiAffinity {
                    preferred_during_scheduling_ignored_during_execution: Some(vec![
                        WeightedPodAffinityTerm {
                            pod_affinity_term: PodAffinityTerm {
                                label_selector: Some(LabelSelector {
                                    match_expressions: None,
                                    match_labels: Some(BTreeMap::from([
                                        ("app.kubernetes.io/name".to_string(), "kafka".to_string(),),
                                        (
                                            "app.kubernetes.io/instance".to_string(),
                                            "simple-kafka".to_string(),
                                        ),
                                        (
                                            "app.kubernetes.io/component".to_string(),
                                            "broker".to_string(),
                                        )
                                    ]))
                                }),
                                namespace_selector: None,
                                namespaces: None,
                                topology_key: TOPOLOGY_KEY_HOSTNAME.to_string(),
                            },
                            weight: 70
                        }
                    ]),
                    required_during_scheduling_ignored_during_execution: None,
                }),
                node_affinity: Some(NodeAffinity {
                    preferred_during_scheduling_ignored_during_execution: None,
                    required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                        node_selector_terms: vec![NodeSelectorTerm {
                            match_expressions: Some(vec![NodeSelectorRequirement {
                                key: "topology.kubernetes.io/zone".to_string(),
                                operator: "In".to_string(),
                                values: Some(vec![
                                    "antarctica-east1".to_string(),
                                    "antarctica-west1".to_string()
                                ]),
                            }]),
                            match_fields: None,
                        }]
                    }),
                }),
                node_selector: Some(StackableNodeSelector {
                    node_selector: BTreeMap::from([("disktype".to_string(), "ssd".to_string())])
                }),
            }
        );
    }

    #[test]
    fn test_affinity_merge_overwrite_existing_attribute() {
        let default_affinity = StackableAffinityFragment {
            pod_affinity: None,
            pod_anti_affinity: Some(PodAntiAffinity {
                preferred_during_scheduling_ignored_during_execution: Some(vec![
                    affinity_between_role_pods("kafka", "simple-kafka", "broker", 70),
                ]),
                required_during_scheduling_ignored_during_execution: None,
            }),
            node_affinity: None,
            node_selector: None,
        };

        // The following anti-affinity tells k8s it *must* spread the brokers across multiple zones
        // It will overwrite the default anti-affinity
        let role_input = r#"
          podAntiAffinity:
            requiredDuringSchedulingIgnoredDuringExecution:
              - labelSelector:
                  matchLabels:
                    app.kubernetes.io/name: kafka
                    app.kubernetes.io/instance: simple-kafka
                    app.kubernetes.io/component: broker
                topologyKey: topology.kubernetes.io/zone
          "#;
        let mut role_affinity: StackableAffinityFragment =
            serde_yaml::from_str(role_input).expect("illegal test input");

        role_affinity.merge(&default_affinity);
        let merged_affinity: StackableAffinity = fragment::validate(role_affinity).unwrap();

        assert_eq!(
            merged_affinity,
            StackableAffinity {
                pod_affinity: None,
                pod_anti_affinity: Some(PodAntiAffinity {
                    preferred_during_scheduling_ignored_during_execution: None,
                    required_during_scheduling_ignored_during_execution: Some(vec![
                        PodAffinityTerm {
                            label_selector: Some(LabelSelector {
                                match_expressions: None,
                                match_labels: Some(BTreeMap::from([
                                    ("app.kubernetes.io/name".to_string(), "kafka".to_string(),),
                                    (
                                        "app.kubernetes.io/instance".to_string(),
                                        "simple-kafka".to_string(),
                                    ),
                                    (
                                        "app.kubernetes.io/component".to_string(),
                                        "broker".to_string(),
                                    )
                                ]))
                            }),
                            namespace_selector: None,
                            namespaces: None,
                            topology_key: "topology.kubernetes.io/zone".to_string(),
                        }
                    ]),
                }),
                node_affinity: None,
                node_selector: None,
            }
        );
    }

    #[test]
    fn test_affinity_between_role_pods() {
        let app_name = "kafka";
        let cluster_name = "simple-kafka";
        let role = "broker";

        let anti_affinity = affinity_between_role_pods(app_name, cluster_name, role, 70);
        assert_eq!(
            anti_affinity,
            WeightedPodAffinityTerm {
                pod_affinity_term: PodAffinityTerm {
                    label_selector: Some(LabelSelector {
                        match_expressions: None,
                        match_labels: Some(BTreeMap::from([
                            ("app.kubernetes.io/name".to_string(), "kafka".to_string(),),
                            (
                                "app.kubernetes.io/instance".to_string(),
                                "simple-kafka".to_string(),
                            ),
                            (
                                "app.kubernetes.io/component".to_string(),
                                "broker".to_string(),
                            )
                        ]))
                    }),
                    namespace_selector: None,
                    namespaces: None,
                    topology_key: TOPOLOGY_KEY_HOSTNAME.to_string(),
                },
                weight: 70
            }
        );
    }

    #[test]
    fn test_affinity_between_cluster_pods() {
        let app_name = "kafka";
        let cluster_name = "simple-kafka";

        let anti_affinity = affinity_between_cluster_pods(app_name, cluster_name, 20);
        assert_eq!(
            anti_affinity,
            WeightedPodAffinityTerm {
                pod_affinity_term: PodAffinityTerm {
                    label_selector: Some(LabelSelector {
                        match_expressions: None,
                        match_labels: Some(BTreeMap::from([
                            ("app.kubernetes.io/name".to_string(), "kafka".to_string(),),
                            (
                                "app.kubernetes.io/instance".to_string(),
                                "simple-kafka".to_string(),
                            )
                        ]))
                    }),
                    namespace_selector: None,
                    namespaces: None,
                    topology_key: TOPOLOGY_KEY_HOSTNAME.to_string(),
                },
                weight: 20
            }
        );
    }
}
