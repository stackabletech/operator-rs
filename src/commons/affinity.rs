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
    kvp::consts::{COMPONENT_KEY, INSTANCE_KEY, NAME_KEY},
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
        tracing::warn!("Deprecated field `selector` was specified. Please use the new `affinity` field instead, as support for `selector` will be removed in the future. See https://docs.stackable.tech/home/stable/contributor/adr/ADR026-affinities.html for details");

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

        if let Some(node_labels) = node_labels {
            self.node_selector
                .get_or_insert(StackableNodeSelector {
                    node_selector: BTreeMap::new(),
                })
                .node_selector
                .extend(node_labels);
        }

        if let Some(NodeAffinity {
            required_during_scheduling_ignored_during_execution:
                Some(NodeSelector {
                    node_selector_terms,
                }),
            ..
        }) = node_affinity
        {
            self.node_affinity
                .get_or_insert(NodeAffinity {
                    preferred_during_scheduling_ignored_during_execution: None,
                    required_during_scheduling_ignored_during_execution: None,
                })
                .required_during_scheduling_ignored_during_execution
                .get_or_insert(NodeSelector {
                    node_selector_terms: Vec::new(),
                })
                .node_selector_terms
                .extend(node_selector_terms);
        }
    }
}

#[derive(Clone, Debug, Eq, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackableNodeSelector {
    #[serde(flatten)]
    pub node_selector: BTreeMap<String, String>,
}

impl Atomic for StackableNodeSelector {}

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
                    (NAME_KEY.to_string(), app_name.to_string()),
                    (INSTANCE_KEY.to_string(), cluster_name.to_string()),
                    (COMPONENT_KEY.to_string(), role.to_string()),
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
                    (NAME_KEY.to_string(), app_name.to_string()),
                    (INSTANCE_KEY.to_string(), cluster_name.to_string()),
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
    use rstest::rstest;

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

    #[rstest]
    #[case::legacy_selector_labels_specified(
        r#"
    matchLabels:
      disktype: ssd
    "#,
        r#"
    nodeSelector:
      generation: new
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#,
        r#"
    nodeSelector:
      disktype: ssd
      generation: new
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#
    )]
    #[case::legacy_selector_expression_specified(
        r#"
    matchExpressions:
        - key: topology.kubernetes.io/continent
          operator: In
          values:
            - europe
    "#,
        r#"
    nodeSelector:
      generation: new
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#,
        r#"
    nodeSelector:
      generation: new
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
          - matchExpressions:
            - key: topology.kubernetes.io/continent
              operator: In
              values:
                - europe

    "#
    )]
    #[case::legacy_selector_expression_and_labels_specified(
        r#"
    matchLabels:
      disktype: ssd
    matchExpressions:
        - key: topology.kubernetes.io/continent
          operator: In
          values:
            - europe
    "#,
        r#"
    nodeSelector:
      generation: new
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#,
        r#"
    nodeSelector:
      generation: new
      disktype: ssd
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
          - matchExpressions:
            - key: topology.kubernetes.io/continent
              operator: In
              values:
                - europe

    "#
    )]
    #[case::legacy_selector_labels_no_new_labels(
        r#"
    matchLabels:
      disktype: ssd
    "#,
        r#"
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#,
        r#"
    nodeSelector:
      disktype: ssd
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#
    )]
    #[case::legacy_selector_labels_no_new_labels(
        r#"
    matchExpressions:
        - key: topology.kubernetes.io/zone
          operator: In
          values:
            - africa-east1
    "#,
        r#"
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
    "#,
        r#"
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - antarctica-east1
                - antarctica-west1
          - matchExpressions:
            - key: topology.kubernetes.io/zone
              operator: In
              values:
                - africa-east1
    "#
    )]
    fn test_add_legacy_selector_fn(
        #[case] legacy_selector: &str,
        #[case] new_selector: &str,
        #[case] expected_result: &str,
    ) {
        let legacy_selector: LabelSelector =
            serde_yaml::from_str(legacy_selector).expect("illegal test input for legacy selector");

        let mut new_selector: StackableAffinityFragment =
            serde_yaml::from_str(new_selector).expect("illegal test input for new selector");

        let expected_result: StackableAffinityFragment =
            serde_yaml::from_str(expected_result).expect("illegal test input for expected result");

        // Merge legacy and new node selectors
        #[allow(deprecated)]
        new_selector.add_legacy_selector(&legacy_selector);

        assert_eq!(expected_result, new_selector);
    }
}
