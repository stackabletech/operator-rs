use std::collections::BTreeMap;

use k8s_openapi::{
    api::core::v1::{
        NodeAffinity, NodeSelector, NodeSelectorRequirement, NodeSelectorTerm, PodAffinity,
        PodAffinityTerm, PodAntiAffinity, WeightedPodAffinityTerm,
    },
    apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_operator_derive::Fragment;

use crate::{
    config::merge::{Atomic, Merge},
    labels::{APP_COMPONENT_LABEL, APP_INSTANCE_LABEL, APP_NAME_LABEL},
};

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
    pub node_selector: Option<StackableNodeSelector>,
}

impl StackableAffinityFragment {
    #[deprecated(
        since = "0.36.0",
        note = "During https://github.com/stackabletech/issues/issues/323 we moved from the previous selector field on a rolegroup to a more generic affinity handling. \
We still need to support the old selector field, which has some custom magic (see the code in this function). \
So we need a way to transform the old into the mechanism which this function offers. \
It will be removed once we stop supporting the old mechanism."
    )]
    pub fn add_legacy_selector(&mut self, label_selector: &LabelSelector) {
        let node_labels = label_selector.match_labels.clone();
        let node_label_exprs = label_selector.match_expressions.clone();

        let node_affinity = node_label_exprs.map(|node_label_exprs| NodeAffinity {
            required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                node_selector_terms: vec![NodeSelectorTerm {
                    match_expressions: Some(
                        node_label_exprs
                            .into_iter()
                            .map(
                                |LabelSelectorRequirement {
                                     key,
                                     operator,
                                     values,
                                 }| {
                                    NodeSelectorRequirement {
                                        key,
                                        operator,
                                        values,
                                    }
                                },
                            )
                            .collect(),
                    ),
                    ..NodeSelectorTerm::default()
                }],
            }),
            ..NodeAffinity::default()
        });

        self.node_selector = node_labels.map(|node_labels| StackableNodeSelector {
            node_selector: node_labels,
        });
        self.node_affinity = node_affinity;
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackableNodeSelector {
    #[serde(flatten)]
    pub node_selector: BTreeMap<String, String>,
}

impl Atomic for StackableNodeSelector {}

pub fn anti_affinity_between_role_pods(
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
                    (APP_NAME_LABEL.to_string(), app_name.to_string()),
                    (APP_INSTANCE_LABEL.to_string(), cluster_name.to_string()),
                    (APP_COMPONENT_LABEL.to_string(), role.to_string()),
                    // We don't include the role-group label here, as the anti-affinity should be between all rolegroups of the given role
                ])),
            }),
            namespace_selector: None,
            namespaces: None,
            topology_key: "kubernetes.io/hostname".to_string(),
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
                    anti_affinity_between_role_pods("kafka", "simple-kafka", "broker", 70),
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
                                topology_key: "kubernetes.io/hostname".to_string(),
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
                    anti_affinity_between_role_pods("kafka", "simple-kafka", "broker", 70),
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
    fn test_anti_affinity_between_role_pods() {
        let app_name = "kafka";
        let cluster_name = "simple-kafka";
        let role = "broker";

        let anti_affinity = anti_affinity_between_role_pods(app_name, cluster_name, role, 50);
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
                    topology_key: "kubernetes.io/hostname".to_string(),
                },
                weight: 50
            }
        );
    }
}
