//! This module provides utility functions for dealing with role (types) and role groups.
//!
//! While other modules in this crate try to be generic and reusable for other operators this one makes very specific
//! assumptions about how a CRD is structured.
//!
//! These assumptions are detailed and explained below.
//!
//! ## Roles / Role types
//!
//! A CRD is often used to operate another piece of software.
//! Software - especially the distributed kind - sometimes consists of multiple different types of program working
//! together to achieve their goal. These different types are what we call a _role_.
//!
//! ## Examples
//!
//! ### Apache Hadoop HDFS
//!
//! - NameNode
//! - DataNode
//! - JournalNode
//!
//! ### Kubernetes
//!
//! - kube-apiserver
//! - kubelet
//! - kube-controller-manager
//! - ...
//!
//! ## Role Groups
//!
//! There is sometimes a need to have different configuration options or different label selectors for different replicas of the same role.
//! Role groups are what allows this.
//! Nested under a role there can be multiple role groups, each with its own LabelSelector and configuration.
//!
//! ### Example
//!
//! This example has one role (`leader`) and two role groups (`default`, and `20core`)
//!
//! ```yaml
//!   leader:
//!     roleGroups:
//!       default:
//!         selector:
//!           matchLabels:
//!             component: spark
//!           matchExpressions:
//!             - { key: tier, operator: In, values: [ cache ] }
//!             - { key: environment, operator: NotIn, values: [ dev ] }
//!         config:
//!           cores: 1
//!           memory: "1g"
//!         replicas: 3
//!       20core:
//!         selector:
//!           matchLabels:
//!             component: spark
//!             cores: 20
//!           matchExpressions:
//!             - { key: tier, operator: In, values: [ cache ] }
//!             - { key: environment, operator: NotIn, values: [ dev ] }
//!           config:
//!             cores: 10
//!             memory: "1g"
//!           replicas: 3
//!     config:
//! ```
//!
//! ## Pod labels
//!
//! Each Pod that Operators create needs to have a common set of labels.
//! These labels are (with one exception) listed in the Kubernetes [documentation](https://kubernetes.io/docs/concepts/overview/working-with-objects/common-labels/):
//!
//! - `app.kubernetes.io/name`: The name of the application. This will usually be a static string (e.g. "zookeeper").
//! - `app.kubernetes.io/instance`: The name of the parent resource, this is useful so an operator can list all its pods by using a LabelSelector
//! - `app.kubernetes.io/version`: The current version of the application
//! - `app.kubernetes.io/component`: The role/role type, this is used to distinguish multiple pods on the same node from each other
//! - `app.kubernetes.io/part-of`: The name of a higher level application this one is part of. We have decided to leave this empty for now.
//! - `app.kubernetes.io/managed-by`: The tool being used to manage the operation of an application (e.g. "zookeeper-operator")
//! - `app.kubernetes.io/role-group`: The name of the role group this pod belongs to
//!
//! NOTE: We find the official description to be ambiguous so we use these labels as defined above.
//!
//! Each resource can have more operator specific labels.

use std::collections::{BTreeMap, HashMap};

use k8s_openapi::api::core::v1::PodTemplateSpec;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "crds")]
use crate::versioned::versioned;
use crate::{commons::pdb::PdbConfig, utils::crds::raw_object_schema};

mod java;
pub use java::{Error, JavaCommonConfig, JvmArgumentOverrides};

mod utils;
pub use utils::*;

#[cfg(feature = "crds")]
mod v1alpha2_impl;

#[cfg(feature = "crds")]
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
pub mod versioned {
    /// This struct represents a role - e.g. HDFS datanodes or Trino workers.
    ///
    /// It has a key-value-map containing all the roleGroups that are part of this role. Additionally, there is a
    /// `config`, which is configurable at the role **and** roleGroup level. Everything at roleGroup level is merged on
    /// top of what is configured on role level.
    ///
    /// There is also a second form of config, which can only be configured at role level, the `roleConfig`. You can
    /// learn more about this in the [Roles and role group concept documentation][1].
    ///
    /// [1]: DOCS_BASE_URL_PLACEHOLDER/concepts/roles-and-role-groups
    //
    // Everything below is only a "normal" comment, not rustdoc - so we don't bloat the CRD documentation
    // with technical (Rust) details.
    //
    // `Config` here is the `config` shared between role and roleGroup.
    //
    // `RoleConfig` here is the `roleConfig` only available on the role. It defaults to [`GenericRoleConfig`], which is
    // sufficient for most of the products. There are some exceptions, where e.g. [`EmptyRoleConfig`] is used.
    // However, product-operators can define their own - custom - struct and use that here.
    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Role<
        Config,
        ConfigOverrides,
        RoleConfig = GenericRoleConfig,
        CommonConfig = GenericCommonConfig,
    >
    where
        // Don't remove this trait bounds!!!
        // We don't know why, but if you remove either of them, the generated default value in the CRDs will
        // be missing!
        RoleConfig: Default + JsonSchema + Serialize,
        CommonConfig: Default + JsonSchema + Serialize,
        ConfigOverrides: Default + JsonSchema + Serialize,
    {
        #[serde(
            flatten,
            bound(
                deserialize = "Config: Default + Deserialize<'de>, CommonConfig: Deserialize<'de>, ConfigOverrides: Deserialize<'de>"
            )
        )]
        pub config: CommonConfiguration<Config, CommonConfig, ConfigOverrides>,

        #[serde(default)]
        pub role_config: RoleConfig,

        /// The set of role groups for this role, keyed by their name.
        ///
        /// A role group is a subset of the replicas of a role that share the same configuration,
        /// allowing finer-grained control than the role level. This is useful to e.g. schedule groups
        /// onto different classes of nodes or into different regions, or to run them with different
        /// settings. Configuration set on a role group is merged on top of the role-level `config`,
        /// with the more specific role group values taking precedence.
        ///
        /// Every role needs at least one role group. A role with a single role group conventionally
        /// names it `default`.
        ///
        /// Read the
        /// [roles and role groups concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/roles-and-role-groups)
        /// for more details.
        #[versioned(
            changed(
                since = "v1alpha2",
                from_type = "HashMap<String, v1alpha1::RoleGroup<Config, CommonConfig, ConfigOverrides>>"
            ),
            hint(map)
        )]
        pub role_groups:
            HashMap<String, v1alpha2::RoleGroup<Config, CommonConfig, ConfigOverrides>>,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[serde(
        rename_all = "camelCase",
        bound(
            deserialize = "Config: Default + Deserialize<'de>, CommonConfig: Default + Deserialize<'de>, ConfigOverrides: Default + Deserialize<'de>"
        )
    )]
    #[schemars(
        bound = "Config: JsonSchema, CommonConfig: JsonSchema, ConfigOverrides: Default + JsonSchema"
    )]
    pub struct RoleGroup<Config, CommonConfig, ConfigOverrides> {
        #[serde(flatten)]
        pub config: CommonConfiguration<Config, CommonConfig, ConfigOverrides>,

        #[versioned(changed(since = "v1alpha2", from_type = "Option<u16>"))]
        pub replicas: Replicas,
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(
        deserialize = "Config: Default + Deserialize<'de>, CommonConfig: Default + Deserialize<'de>, ConfigOverrides: Default + Deserialize<'de>"
    )
)]
#[schemars(
    bound = "Config: JsonSchema, CommonConfig: JsonSchema, ConfigOverrides: Default + JsonSchema"
)]
pub struct CommonConfiguration<Config, CommonConfig, ConfigOverrides> {
    #[serde(default)]
    // We can't depend on Config being `Default`, since that trait is not object-safe
    // We only need to generate schemas for fully specified types, but schemars_derive
    // does not support specifying custom bounds.
    #[schemars(default = "Self::default_config")]
    pub config: Config,

    /// The `configOverrides` can be used to configure properties in product config files
    /// that are not exposed in the CRD. Read the
    /// [config overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#config-overrides)
    /// and consult the operator specific usage guide documentation for details on the
    /// available config files and settings for the specific product.
    #[serde(default)]
    pub config_overrides: ConfigOverrides,

    /// `envOverrides` configure environment variables to be set in the Pods.
    /// It is a map from strings to strings - environment variables and the value to set.
    /// Read the
    /// [environment variable overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#env-overrides)
    /// for more information and consult the operator specific usage guide to find out about
    /// the product specific environment variables that are available.
    #[serde(default)]
    pub env_overrides: HashMap<String, String>,

    // BTreeMap to keep some order with the cli arguments.
    // TODO add documentation.
    #[serde(default)]
    pub cli_overrides: BTreeMap<String, String>,

    /// In the `podOverrides` property you can define a
    /// [PodTemplateSpec](https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.34/#podtemplatespec-v1-core)
    /// to override any property that can be set on a Kubernetes Pod.
    /// Read the
    /// [Pod overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#pod-overrides)
    /// for more information.
    #[serde(default)]
    #[schemars(schema_with = "raw_object_schema")]
    pub pod_overrides: PodTemplateSpec,

    // No docs needed, as we flatten this struct.
    //
    // This field is product-specific and can contain e.g. jvmArgumentOverrides.
    //
    // If [`JavaCommonConfig`] is used, please use [`Role::get_merged_jvm_argument_overrides`] instead of
    // reading this field directly.
    #[serde(flatten, default)]
    pub product_specific_common_config: CommonConfig,
}

impl<Config, CommonConfig, ConfigOverrides>
    CommonConfiguration<Config, CommonConfig, ConfigOverrides>
{
    fn default_config() -> serde_json::Value {
        serde_json::json!({})
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct GenericCommonConfig {}

/// This is a product-agnostic RoleConfig, which is sufficient for most of the products.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericRoleConfig {
    #[serde(default)]
    pub pod_disruption_budget: PdbConfig,
}

/// This is a product-agnostic RoleConfig, with nothing in it. It is used e.g. by products that have
/// nothing configurable at role level.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyRoleConfig {}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Replicas {
    /// Use a fixed number of replicas.
    //
    // We sadly cannot simply wrap a u16 because Kubernetes doesn't support mixing enum variants
    // with the shape of an object AND primitive types, like a u16 in this case.
    Fixed { count: u16 },

    /// Use a fully managed, auto-scaled number of replicas.
    Managed {},

    /// Use a auto-scaled number of replicas managed by external/custom mechanisms, e.g. HPA.
    Custom {},
}

impl From<Option<u16>> for Replicas {
    fn from(replicas: Option<u16>) -> Self {
        match replicas {
            Some(count) => Self::Fixed { count },
            None => Self::Custom {},
        }
    }
}

impl From<Replicas> for Option<u16> {
    fn from(replicas: Replicas) -> Self {
        match replicas {
            Replicas::Fixed { count } => Some(count),
            Replicas::Managed {} => None,
            // FIXME (@Techassi): Deal with this (we need fallible conversions or we need to support
            // tracking type changes). Alternatively, we could map this to None as well, but that
            // would be a lossy conversion.
            Replicas::Custom {} => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replica_counts_with_all_replicas_set() {
        let replicas = [Some(3), Some(2), Some(5)];

        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsZero),
            Some(10)
        );
        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsNone),
            Some(10)
        );
        assert_eq!(estimated_replica_count(replicas), 10);
    }

    #[test]
    fn replica_counts_with_one_replica_unset() {
        let replicas = [Some(3), None, Some(2)];

        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsZero),
            None
        );
        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsNone),
            None
        );
        assert_eq!(estimated_replica_count(replicas), 6);
    }

    #[test]
    fn replica_counts_with_a_zero_replica() {
        let replicas = [Some(3), Some(0)];

        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsZero),
            Some(3)
        );
        // With treat_zero_as_none the zero turns the whole count into None.
        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsNone),
            None
        );
        assert_eq!(estimated_replica_count(replicas), 3);
    }

    #[test]
    fn replica_counts_with_a_single_zero_role_groups_group() {
        let replicas = [Some(0)];

        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsZero),
            Some(0)
        );
        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsNone),
            None
        );
        assert_eq!(estimated_replica_count(replicas), 0);
    }

    #[test]
    fn replica_counts_without_role_groups() {
        let replicas = [];

        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsZero),
            Some(0)
        );
        assert_eq!(
            fixed_replica_count(replicas, ZeroReplicasCounting::TreatAsNone),
            None
        );
        assert_eq!(estimated_replica_count(replicas), 0);
    }
}
