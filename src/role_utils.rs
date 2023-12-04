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
//! There is sometimes a need to have different configuration options or different label selectors for different replicas of the same role.
//! Role groups are what allows this.
//! Nested under a role there can be multiple role groups, each with its own LabelSelector and configuration.
//!
//! ## Example
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
//! # Pod labels
//!
//! Each Pod that Operators create needs to have a common set of labels.
//! These labels are (with one exception) listed in the Kubernetes [documentation](https://kubernetes.io/docs/concepts/overview/working-with-objects/common-labels/):
//!
//! * app.kubernetes.io/name - The name of the application. This will usually be a static string (e.g. "zookeeper").
//! * app.kubernetes.io/instance - The name of the parent resource, this is useful so an operator can list all its pods by using a LabelSelector
//! * app.kubernetes.io/version - The current version of the application
//! * app.kubernetes.io/component - The role/role type, this is used to distinguish multiple pods on the same node from each other
//! * app.kubernetes.io/part-of - The name of a higher level application this one is part of. We have decided to leave this empty for now.
//! * app.kubernetes.io/managed-by - The tool being used to manage the operation of an application (e.g. "zookeeper-operator")
//! * app.kubernetes.io/role-group - The name of the role group this pod belongs to
//!
//! NOTE: We find the official description to be ambiguous so we use these labels as defined above.
//!
//! Each resource can have more operator specific labels.

use std::{
    collections::{BTreeMap, HashMap},
    fmt::{Debug, Display},
};

use crate::{
    commons::pdb::PdbConfig,
    config::{
        fragment::{self, FromFragment},
        merge::Merge,
    },
    product_config_utils::Configuration,
};
use derivative::Derivative;
use k8s_openapi::{
    api::core::v1::PodTemplateSpec, apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{runtime::reflector::ObjectRef, Resource};
use schemars::{schema::Schema, visit::Visitor, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "T: Default + Deserialize<'de>")
)]
pub struct CommonConfiguration<T> {
    #[serde(default)]
    // We can't depend on T being `Default`, since that trait is not object-safe
    // We only need to generate schemas for fully specified types, but schemars_derive
    // does not support specifying custom bounds.
    #[schemars(default = "config_schema_default")]
    pub config: T,
    /// The `configOverrides` can be used to configure properties in product config files
    /// that are not exposed in the CRD. Read the
    /// [config overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#config-overrides)
    /// and consult the operator specific usage guide documentation for details on the
    /// available config files and settings for the specific product.
    #[serde(default)]
    pub config_overrides: HashMap<String, HashMap<String, String>>,
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
    /// [PodTemplateSpec](https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.27/#podtemplatespec-v1-core)
    /// to override any property that can be set on a Kubernetes Pod.
    /// Read the
    /// [Pod overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#pod-overrides)
    /// for more information.
    #[serde(default)]
    #[schemars(schema_with = "pod_overrides_schema")]
    pub pod_overrides: PodTemplateSpec,
}

/// Simplified schema for PodTemplateSpec without mandatory fields (e.g. `containers`) or documentation.
///
/// The normal PodTemplateSpec requires you to specify `containers` as an `Vec<Container>`.
/// Often times the user want's to overwrite/add stuff not related to a container
/// (e.g. tolerations or a ServiceAccount), so it's annoying that he always needs to
/// specify an empty array for `containers`.
///
/// Additionally all docs are removed, as the resulting Stackable CRD objects where to big for Kubernetes.
/// E.g. the HdfsCluster CRD increased to ~3.2 MB (which is over the limit of 3MB), after stripping
/// the docs it went down to ~1.3 MiB.
pub fn pod_overrides_schema(gen: &mut schemars::gen::SchemaGenerator) -> Schema {
    let mut schema = PodTemplateSpec::json_schema(gen);
    SimplifyOverrideSchema.visit_schema(&mut schema);
    if let Schema::Object(schema) = &mut schema {
        let meta = schema.metadata.get_or_insert_with(Default::default);
        meta.description = Some("See PodTemplateSpec (https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.27/#podtemplatespec-v1-core) for more details".to_string());
    }
    schema
}

struct SimplifyOverrideSchema;
impl schemars::visit::Visitor for SimplifyOverrideSchema {
    fn visit_schema_object(&mut self, schema: &mut schemars::schema::SchemaObject) {
        // Strip docs to make the schema more compact
        if let Some(meta) = &mut schema.metadata {
            meta.description = None;
            meta.examples.clear();
        }
        // Make all options optional
        if let Some(object) = &mut schema.object {
            object.required.clear();
        }
        schemars::visit::visit_schema_object(self, schema);
    }
}

fn config_schema_default() -> serde_json::Value {
    serde_json::json!({})
}

/// This struct represents a role - e.g. HDFS datanodes or Trino workers. It has a [`HashMap`] containing
/// all the roleGroups that are part of this role. Additionally, there is a `config`, which is configurable
/// at the role *and* roleGroup level. Everything at roleGroup level is merged on top of what is configured
/// on role level using the [`Merge`] trait. There is also a second form of config, which can only be configured
/// at role level, the `roleConfig`.
//
// Everything below is only a "normal" comment, not rustdoc - so we don't bloat the CRD documentation
// with technical (Rust) details.
//
// `T` here is the `config` shared between role and roleGroup.
//
// `U` here is the `roleConfig` only available on the role. It defaults to [`GenericRoleConfig`], which is
// sufficient for most of the products. There are some exceptions, where e.g. [`EmptyRoleConfig`] is used.
// However, product-operators can define their own - custom - struct and use that here.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Role<T, U = GenericRoleConfig>
where
    // Don't remove this trait bounds!!!
    // We don't know why, but if you remove either of them, the generated default value in the CRDs will
    // be missing!
    U: Default + JsonSchema + Serialize,
{
    #[serde(flatten, bound(deserialize = "T: Default + Deserialize<'de>"))]
    pub config: CommonConfiguration<T>,

    #[serde(default)]
    pub role_config: U,

    pub role_groups: HashMap<String, RoleGroup<T>>,
}

impl<T, U> Role<T, U>
where
    T: Configuration + 'static,
    U: Default + JsonSchema + Serialize,
{
    /// This casts a generic struct implementing [`crate::product_config_utils::Configuration`]
    /// and used in [`Role`] into a Box of a dynamically dispatched
    /// [`crate::product_config_utils::Configuration`] Trait. This is required to use the generic
    /// [`Role`] with more than a single generic struct. For example different roles most likely
    /// have different structs implementing Configuration.
    pub fn erase(self) -> Role<Box<dyn Configuration<Configurable = T::Configurable>>, U> {
        Role {
            config: CommonConfiguration {
                config: Box::new(self.config.config)
                    as Box<dyn Configuration<Configurable = T::Configurable>>,
                config_overrides: self.config.config_overrides,
                env_overrides: self.config.env_overrides,
                cli_overrides: self.config.cli_overrides,
                pod_overrides: self.config.pod_overrides,
            },
            role_config: self.role_config,
            role_groups: self
                .role_groups
                .into_iter()
                .map(|(name, group)| {
                    (
                        name,
                        RoleGroup {
                            config: CommonConfiguration {
                                config: Box::new(group.config.config)
                                    as Box<dyn Configuration<Configurable = T::Configurable>>,
                                config_overrides: group.config.config_overrides,
                                env_overrides: group.config.env_overrides,
                                cli_overrides: group.config.cli_overrides,
                                pod_overrides: group.config.pod_overrides,
                            },
                            replicas: group.replicas,
                            selector: group.selector,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// This is a product-agnostic RoleConfig, which is sufficient for most of the products.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericRoleConfig {
    #[serde(default)]
    pub pod_disruption_budget: PdbConfig,
}

/// This is a product-agnostic RoleConfig, with nothing in it. It is used e.g. by products that have
/// nothing configurable at role level.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyRoleConfig {}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "T: Default + Deserialize<'de>")
)]
pub struct RoleGroup<T> {
    #[serde(flatten)]
    pub config: CommonConfiguration<T>,
    pub replicas: Option<u16>,
    // TODO Can be removed after we stop supporting this field.
    // See ADR 26 Affinities
    pub selector: Option<LabelSelector>,
}

impl<T> RoleGroup<T> {
    pub fn validate_config<C, U>(
        &self,
        role: &Role<T, U>,
        default_config: &T,
    ) -> Result<C, fragment::ValidationError>
    where
        C: FromFragment<Fragment = T>,
        T: Merge + Clone,
        U: Default + JsonSchema + Serialize,
    {
        let mut role_config = role.config.config.clone();
        role_config.merge(default_config);
        let mut rolegroup_config = self.config.config.clone();
        rolegroup_config.merge(&role_config);
        fragment::validate(rolegroup_config)
    }
}

/// A reference to a named role group of a given cluster object
#[derive(Derivative)]
#[derivative(
    Debug(bound = "K::DynamicType: Debug"),
    Clone(bound = "K::DynamicType: Clone")
)]
pub struct RoleGroupRef<K: Resource> {
    pub cluster: ObjectRef<K>,
    pub role: String,
    pub role_group: String,
}

impl<K: Resource> RoleGroupRef<K> {
    pub fn object_name(&self) -> String {
        format!("{}-{}-{}", self.cluster.name, self.role, self.role_group)
    }
}

impl<K: Resource> Display for RoleGroupRef<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "role group {}/{} of {}",
            self.role, self.role_group, self.cluster
        ))
    }
}
