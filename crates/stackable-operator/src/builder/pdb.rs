use k8s_openapi::{
    api::policy::v1::{PodDisruptionBudget, PodDisruptionBudgetSpec},
    apimachinery::pkg::{
        apis::meta::v1::{LabelSelector, ObjectMeta},
        util::intstr::IntOrString,
    },
};
use kube::{Resource, ResourceExt};
use snafu::{ResultExt, Snafu};

use crate::{
    builder::meta::ObjectMetaBuilder,
    kvp::{Label, Labels},
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("failed to create role selector labels"))]
    RoleSelectorLabels { source: crate::kvp::LabelError },

    #[snafu(display("failed to set owner reference from resource"))]
    OwnerReferenceFromResource { source: crate::builder::meta::Error },

    #[snafu(display("failed to create app.kubernetes.io/managed-by label"))]
    ManagedByLabel { source: crate::kvp::LabelError },
}

/// This builder is used to construct [`PodDisruptionBudget`]s.
/// If you are using this to create [`PodDisruptionBudget`]s according to [ADR 30 on Allowed Pod disruptions][adr],
/// the use of [`PodDisruptionBudgetBuilder::new_with_role`] is recommended.
///
/// The following attributes on a [`PodDisruptionBudget`] are considered mandatory and must be specified
/// before being able to construct the [`PodDisruptionBudget`]:
///
/// 1. [`PodDisruptionBudget::metadata`]
/// 2. [`PodDisruptionBudgetSpec::selector`]
/// 3. Either [`PodDisruptionBudgetSpec::min_available`] or [`PodDisruptionBudgetSpec::max_unavailable`]
///
/// Both [`PodDisruptionBudget::metadata`] and [`PodDisruptionBudgetSpec::selector`] will be set by [`PodDisruptionBudgetBuilder::new_with_role`].
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

    /// This method populates [`PodDisruptionBudget::metadata`] and
    /// [`PodDisruptionBudgetSpec::selector`] from the give role (not roleGroup!).
    ///
    /// The parameters are the same as the fields from
    /// [`ObjectLabels`][crate::kvp::ObjectLabels]:
    ///
    /// * `owner` - Reference to the k8s object owning the created resource,
    ///   such as `HdfsCluster` or `TrinoCluster`.
    /// * `app_name` - The name of the app being managed, such as `hdfs` or
    ///   `trino`.
    /// * `role` - The role that this object belongs to, e.g. `datanode` or
    ///   `worker`.
    /// * `operator_name` - The DNS-style name of the operator managing the
    ///   object (such as `hdfs.stackable.tech`).
    /// * `controller_name` - The name of the controller inside of the operator
    ///   managing the object (such as `hdfscluster`)
    pub fn new_with_role<T: Resource<DynamicType = ()>>(
        owner: &T,
        app_name: &str,
        role: &str,
        operator_name: &str,
        controller_name: &str,
    ) -> Result<PodDisruptionBudgetBuilder<ObjectMeta, LabelSelector, ()>> {
        let role_selector_labels =
            Labels::role_selector(owner, app_name, role).context(RoleSelectorLabelsSnafu)?;
        let managed_by_label =
            Label::managed_by(operator_name, controller_name).context(ManagedByLabelSnafu)?;
        let metadata = ObjectMetaBuilder::new()
            .namespace_opt(owner.namespace())
            .name(format!("{}-{}", owner.name_any(), role))
            .ownerreference_from_resource(owner, None, Some(true))
            .context(OwnerReferenceFromResourceSnafu)?
            .with_labels(role_selector_labels.clone())
            .with_label(managed_by_label)
            .build();

        Ok(PodDisruptionBudgetBuilder {
            metadata,
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(role_selector_labels.into()),
            },
            ..PodDisruptionBudgetBuilder::default()
        })
    }

    /// Sets the mandatory [`PodDisruptionBudget::metadata`].
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
    /// Sets the mandatory [`PodDisruptionBudgetSpec::selector`].
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
    /// Sets the mandatory [`PodDisruptionBudgetSpec::max_unavailable`].
    /// Mutually exclusive with [`PodDisruptionBudgetBuilder::with_min_available`].
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

    /// Sets the mandatory [`PodDisruptionBudgetSpec::min_available`].
    /// Mutually exclusive with [`PodDisruptionBudgetBuilder::with_max_unavailable`].
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
    /// This function can be called after [`PodDisruptionBudget::metadata`], [`PodDisruptionBudgetSpec::selector`]
    /// and either [`PodDisruptionBudgetSpec::min_available`] or [`PodDisruptionBudgetSpec::max_unavailable`] are set.
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
mod tests {
    use std::collections::BTreeMap;

    use k8s_openapi::{
        api::policy::v1::{PodDisruptionBudget, PodDisruptionBudgetSpec},
        apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString},
    };
    use kube::{CustomResource, core::ObjectMeta};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::builder::meta::{ObjectMetaBuilder, OwnerReferenceBuilder};

    #[test]
    pub fn normal_build() {
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

        assert_eq!(pdb, PodDisruptionBudget {
            metadata: ObjectMeta {
                name: Some("trino".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: Some(PodDisruptionBudgetSpec {
                min_available: Some(IntOrString::Int(42)),
                selector: Some(LabelSelector {
                    match_expressions: None,
                    match_labels: Some(BTreeMap::from([("foo".to_string(), "bar".to_string())])),
                }),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    #[test]
    pub fn build_from_role() {
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

        assert_eq!(pdb, PodDisruptionBudget {
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
                owner_references: Some(vec![
                    OwnerReferenceBuilder::new()
                        .initialize_from_resource(&trino)
                        .block_owner_deletion_opt(None)
                        .controller_opt(Some(true))
                        .build()
                        .unwrap()
                ]),
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
        })
    }
}
