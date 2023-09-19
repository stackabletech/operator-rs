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

#[derive(Debug, Default)]
pub struct PdbBuilder<ObjectMeta, LabelSelector, Constraints> {
    metadata: ObjectMeta,
    selector: LabelSelector,
    /// We intentionally only support fixed numbers, so percentage, see ADR on Pod disruptions for details.
    /// We use u16, as [`IntOrString`] takes an i32 and we don't want to allow negative numbers. u16 will always fit in i32.
    max_unavailable: Option<u16>,
    /// We intentionally only support fixed numbers, so percentage, see ADR on Pod disruptions for details.
    /// We use u16, as [`IntOrString`] takes an i32 and we don't want to allow negative numbers. u16 will always fit in i32.
    min_available: Option<u16>,
    /// Tracks wether either `max_unavailable` or `min_available` are set
    _constraints: Constraints,
}

impl PdbBuilder<(), (), ()> {
    pub fn new() -> Self {
        PdbBuilder::default()
    }

    pub fn new_for_role<T: Resource<DynamicType = ()>>(
        owner: &T,
        app_name: &str,
        role: &str,
        operator_name: &str,
        controller_name: &str,
    ) -> OperatorResult<PdbBuilder<ObjectMeta, LabelSelector, ()>> {
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
        Ok(PdbBuilder {
            metadata,
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(role_selector_labels),
            },
            ..PdbBuilder::default()
        })
    }

    pub fn metadata(self, metadata: impl Into<ObjectMeta>) -> PdbBuilder<ObjectMeta, (), ()> {
        PdbBuilder {
            metadata: metadata.into(),
            selector: self.selector,
            max_unavailable: self.max_unavailable,
            min_available: self.min_available,
            _constraints: self._constraints,
        }
    }
}

impl PdbBuilder<ObjectMeta, (), ()> {
    pub fn selector(self, selector: LabelSelector) -> PdbBuilder<ObjectMeta, LabelSelector, ()> {
        PdbBuilder {
            metadata: self.metadata,
            selector,
            max_unavailable: self.max_unavailable,
            min_available: self.min_available,
            _constraints: self._constraints,
        }
    }
}

impl PdbBuilder<ObjectMeta, LabelSelector, ()> {
    pub fn max_unavailable(
        self,
        max_unavailable: u16,
    ) -> PdbBuilder<ObjectMeta, LabelSelector, bool> {
        PdbBuilder {
            metadata: self.metadata,
            selector: self.selector,
            max_unavailable: Some(max_unavailable),
            min_available: self.min_available,
            _constraints: true, // Some dummy value to set Constraints to something other than ()
        }
    }

    #[deprecated(
        since = "0.51.0",
        note = "It is strongly recommended to use [`max_unavailable`]. Please read the ADR on Pod disruptions before using this function."
    )]
    pub fn min_available(self, min_available: u16) -> PdbBuilder<ObjectMeta, LabelSelector, bool> {
        PdbBuilder {
            metadata: self.metadata,
            selector: self.selector,
            max_unavailable: self.max_unavailable,
            min_available: Some(min_available),
            _constraints: true, // Some dummy value to set Constraints to something other than ()
        }
    }
}

impl PdbBuilder<ObjectMeta, LabelSelector, bool> {
    pub fn build(self) -> PodDisruptionBudget {
        PodDisruptionBudget {
            metadata: self.metadata,
            spec: Some(PodDisruptionBudgetSpec {
                max_unavailable: self.max_unavailable.map(i32::from).map(IntOrString::Int),
                min_available: self.min_available.map(i32::from).map(IntOrString::Int),
                selector: Some(self.selector),
                /// As this is beta as of 1.27 we can not use it yet,
                /// so this builder does not offer this attribute.
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

    use super::PdbBuilder;

    #[test]
    pub fn test_normal_build() {
        let pdb = PdbBuilder::new()
            .metadata(
                ObjectMetaBuilder::new()
                    .namespace("default")
                    .name("trino")
                    .build(),
            )
            .selector(LabelSelector {
                match_expressions: None,
                match_labels: Some(BTreeMap::from([("foo".to_string(), "bar".to_string())])),
            })
            .min_available(42)
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
        let pdb = PdbBuilder::new_for_role(&trino, app_name, role, operator_name, controller_name)
            .unwrap()
            .max_unavailable(2)
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
