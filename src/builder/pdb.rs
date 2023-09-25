use crate::{
    builder::ObjectMetaBuilder,
    error::OperatorResult,
    labels::{role_selector_labels, APP_MANAGED_BY_LABEL},
    utils::format_full_controller_name,
};
use k8s_openapi::{
    api::policy::v1::{PodDisruptionBudget, PodDisruptionBudgetSpec},
    apimachinery::pkg::{
        apis::meta::v1::{LabelSelector, ObjectMeta},
        util::intstr::IntOrString,
    },
};
use kube::{Resource, ResourceExt};

/// This builder is used to construct [`PodDisruptionBudget`]s.
/// If you are using this to create [`PodDisruptionBudget`]s according to [ADR 30 on Allowed Pod disruptions][adr],
/// the use of [`PodDisruptionBudgetBuilder::new_with_role`] is recommended.
///
/// The following attributes on a [`PodDisruptionBudget`] are considered mandatory and must be specified
/// before being able to construct the [`PodDisruptionBudget`]:
///
/// 1. `metadata`
/// 2. `selector`
/// 3. Either `minAvailable` or `maxUnavailable`
///
/// Both `metadata` and `selector` will be set by [`PodDisruptionBudgetBuilder::new_with_role`].
///
/// [adr]: https://docs.stackable.tech/home/stable/contributor/adr/adr030-allowed-pod-disruptions
#[derive(Debug, Default)]
pub struct PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, PodDisruptionBudgetConstraint> {
    metadata: ObjectMeta,
    selector: LabelSelector,
    /// Tracks wether either `maxUnavailable` or `minAvailable` is set.
    constraint: Option<PodDisruptionBudgetConstraint>,
}

/// We intentionally only support fixed numbers, no percentage, see ADR 30 on Pod disruptions for details.
/// We use u16, as [`IntOrString`] takes an i32 and we don't want to allow negative numbers. u16 will always fit in i32.
#[derive(Debug)]
pub enum PodDisruptionBudgetConstraint {
    MaxUnavailable(u16),
    MinAvailable(u16),
}

impl PodDisruptionBudgetBuilder<(), (), ()> {
    pub fn new() -> Self {
        PodDisruptionBudgetBuilder::default()
    }

    /// This method populates `metadata` and `selector` from the give role (not roleGroup!).
    ///
    /// The parameters are the same as the fields from [`crate::labels::ObjectLabels`]:
    /// * `owner` - Reference to the k8s object owning the created resource, such as `HdfsCluster` or `TrinoCluster`.
    /// * `app_name` - The name of the app being managed, such as `hdfs` or `trino`.
    /// * `role` - The role that this object belongs to, e.g. `datanode` or `worker`.
    /// * `operator_name` - The DNS-style name of the operator managing the object (such as `hdfs.stackable.tech`).
    /// * `controller_name` - The name of the controller inside of the operator managing the object (such as `hdfscluster`)
    pub fn new_with_role<T: Resource<DynamicType = ()>>(
        owner: &T,
        app_name: &str,
        role: &str,
        operator_name: &str,
        controller_name: &str,
    ) -> OperatorResult<PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, ()>> {
        let role_selector_labels = role_selector_labels(owner, app_name, role);
        let metadata = ObjectMetaBuilder::new()
            .namespace_opt(owner.namespace())
            .name(format!("{}-{}", owner.name_any(), role))
            .ownerreference_from_resource(owner, None, Some(true))?
            .with_labels(role_selector_labels.clone())
            .with_label(
                APP_MANAGED_BY_LABEL.to_string(),
                format_full_controller_name(operator_name, controller_name),
            )
            .build();

        Ok(PodDisruptionBudgetBuilder {
            metadata,
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(role_selector_labels),
            },
            ..PodDisruptionBudgetBuilder::default()
        })
    }

    pub fn new_with_metadata(
        self,
        metadata: impl Into<ObjectMeta>,
    ) -> PodDisruptionBudgetBuilder<ObjectMeta, (), ()> {
        PodDisruptionBudgetBuilder {
            metadata: metadata.into(),
            ..PodDisruptionBudgetBuilder::default()
        }
    }
}

impl PodDisruptionBudgetBuilder<ObjectMeta, (), ()> {
    pub fn with_selector(
        self,
        selector: LabelSelector,
    ) -> PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, ()> {
        PodDisruptionBudgetBuilder {
            metadata: self.metadata,
            selector,
            constraint: self.constraint,
        }
    }
}

impl PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, ()> {
    pub fn with_max_unavailable(
        self,
        max_unavailable: u16,
    ) -> PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, PodDisruptionBudgetConstraint> {
        PodDisruptionBudgetBuilder {
            metadata: self.metadata,
            selector: self.selector,
            constraint: Some(PodDisruptionBudgetConstraint::MaxUnavailable(
                max_unavailable,
            )),
        }
    }

    #[deprecated(
        since = "0.51.0",
        note = "It is strongly recommended to use [`max_unavailable`]. Please read the ADR on Pod disruptions before using this function."
    )]
    pub fn with_min_available(
        self,
        min_available: u16,
    ) -> PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, PodDisruptionBudgetConstraint> {
        PodDisruptionBudgetBuilder {
            metadata: self.metadata,
            selector: self.selector,
            constraint: Some(PodDisruptionBudgetConstraint::MinAvailable(min_available)),
        }
    }
}

impl PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, PodDisruptionBudgetConstraint> {
    /// This function can be called after `metadata`, `selector` and either `minAvailable` or
    /// `maxUnavailable` are set.
    pub fn build(self) -> PodDisruptionBudget {
        let (max_unavailable, min_available) = match self.constraint {
            Some(PodDisruptionBudgetConstraint::MaxUnavailable(max_unavailable)) => {
                (Some(max_unavailable), None)
            }
            Some(PodDisruptionBudgetConstraint::MinAvailable(min_unavailable)) => {
                (None, Some(min_unavailable))
            }
            None => {
                unreachable!("Either minUnavailable or maxUnavailable must be set at this point!")
            }
        };
        PodDisruptionBudget {
            metadata: self.metadata,
            spec: Some(PodDisruptionBudgetSpec {
                max_unavailable: max_unavailable.map(i32::from).map(IntOrString::Int),
                min_available: min_available.map(i32::from).map(IntOrString::Int),
                selector: Some(self.selector),
                // Because this feature is still in beta in k8s version 1.27, the builder currently does not offer this attribute.
                unhealthy_pod_eviction_policy: Default::default(),
            }),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use k8s_openapi::{
        api::policy::v1::{PodDisruptionBudget, PodDisruptionBudgetSpec},
        apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString},
    };
    use kube::{core::ObjectMeta, CustomResource};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use crate::builder::{ObjectMetaBuilder, OwnerReferenceBuilder};

    use super::PodDisruptionBudgetBuilder;

    #[test]
    pub fn test_normal_build() {
        #[allow(deprecated)]
        let pdb = PodDisruptionBudgetBuilder::new()
            .new_with_metadata(
                ObjectMetaBuilder::new()
                    .namespace("default")
                    .name("trino")
                    .build(),
            )
            .with_selector(LabelSelector {
                match_expressions: None,
                match_labels: Some(BTreeMap::from([("foo".to_string(), "bar".to_string())])),
            })
            .with_min_available(42)
            .build();

        assert_eq!(
            pdb,
            PodDisruptionBudget {
                metadata: ObjectMeta {
                    name: Some("trino".to_string()),
                    namespace: Some("default".to_string()),
                    ..Default::default()
                },
                spec: Some(PodDisruptionBudgetSpec {
                    min_available: Some(IntOrString::Int(42)),
                    selector: Some(LabelSelector {
                        match_expressions: None,
                        match_labels: Some(BTreeMap::from([(
                            "foo".to_string(),
                            "bar".to_string()
                        )])),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }
        )
    }

    #[test]
    pub fn test_build_from_role() {
        #[derive(
            Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize,
        )]
        #[kube(group = "test", version = "v1", kind = "TrinoCluster", namespaced)]
        pub struct TrinoClusterSpec {}
        let trino: TrinoCluster = serde_yaml::from_str(
            "
            apiVersion: test/v1
            kind: TrinoCluster
            metadata:
              name: simple-trino
              namespace: default
              uid: 123 # Needed for the ownerreference
            spec: {}
            ",
        )
        .unwrap();
        let app_name = "trino";
        let role = "worker";
        let operator_name = "trino.stackable.tech";
        let controller_name = "trino-operator-trino-controller";
        let pdb = PodDisruptionBudgetBuilder::new_with_role(
            &trino,
            app_name,
            role,
            operator_name,
            controller_name,
        )
        .unwrap()
        .with_max_unavailable(2)
        .build();

        assert_eq!(
            pdb,
            PodDisruptionBudget {
                metadata: ObjectMeta {
                    name: Some("simple-trino-worker".to_string()),
                    namespace: Some("default".to_string()),
                    labels: Some(BTreeMap::from([
                        ("app.kubernetes.io/name".to_string(), "trino".to_string()),
                        (
                            "app.kubernetes.io/instance".to_string(),
                            "simple-trino".to_string()
                        ),
                        (
                            "app.kubernetes.io/managed-by".to_string(),
                            "trino.stackable.tech_trino-operator-trino-controller".to_string()
                        ),
                        (
                            "app.kubernetes.io/component".to_string(),
                            "worker".to_string()
                        )
                    ])),
                    owner_references: Some(vec![OwnerReferenceBuilder::new()
                        .initialize_from_resource(&trino)
                        .block_owner_deletion_opt(None)
                        .controller_opt(Some(true))
                        .build()
                        .unwrap()]),
                    ..Default::default()
                },
                spec: Some(PodDisruptionBudgetSpec {
                    max_unavailable: Some(IntOrString::Int(2)),
                    selector: Some(LabelSelector {
                        match_expressions: None,
                        match_labels: Some(BTreeMap::from([
                            ("app.kubernetes.io/name".to_string(), "trino".to_string()),
                            (
                                "app.kubernetes.io/instance".to_string(),
                                "simple-trino".to_string()
                            ),
                            (
                                "app.kubernetes.io/component".to_string(),
                                "worker".to_string()
                            )
                        ])),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }
        )
    }
}
