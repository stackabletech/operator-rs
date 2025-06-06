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
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{Debug, Display},
};

use educe::Educe;
use k8s_openapi::api::core::v1::PodTemplateSpec;
use kube::{Resource, runtime::reflector::ObjectRef};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::{
    commons::pdb::PdbConfig,
    config::{
        fragment::{self, FromFragment},
        merge::Merge,
    },
    product_config_utils::Configuration,
    utils::crds::raw_object_schema,
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("missing roleGroup {role_group:?}"))]
    MissingRoleGroup { role_group: String },

    #[snafu(display(
        "Could not parse regex from \"jvmArgumentOverrides.removeRegex\", ignoring it (there might be some added anchors at the start and end): {regex:?}"
    ))]
    InvalidRemoveRegex { source: regex::Error, regex: String },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(
        deserialize = "T: Default + Deserialize<'de>, ProductSpecificCommonConfig: Default + Deserialize<'de>"
    )
)]
pub struct CommonConfiguration<T, ProductSpecificCommonConfig> {
    #[serde(default)]
    // We can't depend on T being `Default`, since that trait is not object-safe
    // We only need to generate schemas for fully specified types, but schemars_derive
    // does not support specifying custom bounds.
    #[schemars(default = "Self::default_config")]
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
    #[schemars(schema_with = "raw_object_schema")]
    pub pod_overrides: PodTemplateSpec,

    // No docs needed, as we flatten this struct.
    //
    // This field is product-specific and can contain e.g. jvmArgumentOverrides.
    //
    // If [`JavaCommonConfig`] is used, please use [`Role::get_merged_jvm_argument_overrides`] instead of
    // reading this field directly.
    #[serde(flatten, default)]
    pub product_specific_common_config: ProductSpecificCommonConfig,
}

impl<T, ProductSpecificCommonConfig> CommonConfiguration<T, ProductSpecificCommonConfig> {
    fn default_config() -> serde_json::Value {
        serde_json::json!({})
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct GenericProductSpecificCommonConfig {}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaCommonConfig {
    /// Allows overriding JVM arguments.
    //
    /// Please read on the [JVM argument overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#jvm-argument-overrides)
    /// for details on the usage.
    #[serde(default)]
    pub jvm_argument_overrides: JvmArgumentOverrides,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JvmArgumentOverrides {
    /// JVM arguments to be added
    #[serde(default)]
    add: Vec<String>,

    /// JVM arguments to be removed by exact match
    //
    // HashSet to be optimized for quick lookup
    #[serde(default)]
    remove: HashSet<String>,

    /// JVM arguments matching any of this regexes will be removed
    #[serde(default)]
    remove_regex: Vec<String>,
}

impl JvmArgumentOverrides {
    pub fn new(add: Vec<String>, remove: HashSet<String>, remove_regex: Vec<String>) -> Self {
        Self {
            add,
            remove,
            remove_regex,
        }
    }

    pub fn new_with_only_additions(add: Vec<String>) -> Self {
        Self {
            add,
            ..Default::default()
        }
    }

    /// Called on **merged** [`JvmArgumentOverrides`}, returns all arguments that should be passed to the JVM.
    ///
    /// **Can only be called on merged config, it will panic otherwise!**
    ///
    ///  We are panicking (instead of returning an Error), because this is not the users fault, but
    /// the operator is doing things wrong
    pub fn effective_jvm_config_after_merging(&self) -> &Vec<String> {
        assert!(
            self.remove.is_empty(),
            "After merging there should be no removals left. \"effective_jvm_config_after_merging\" should only be called on merged configs!"
        );
        assert!(
            self.remove_regex.is_empty(),
            "After merging there should be no removal regexes left. \"effective_jvm_config_after_merging\" should only be called on merged configs!"
        );

        &self.add
    }
}

/// We can not use [`Merge`] here, as this function can fail, e.g. if an invalid regex is specified by the user
impl JvmArgumentOverrides {
    /// Please watch out: Merge order is complicated for this merge.
    /// Test your code!
    pub fn try_merge(&mut self, defaults: &Self) -> Result<(), Error> {
        let regexes = self
            .remove_regex
            .iter()
            .map(|regex| {
                let without_anchors = regex.trim_start_matches('^').trim_end_matches('$');
                let with_anchors = format!("^{without_anchors}$");

                Regex::new(&with_anchors).with_context(|_| InvalidRemoveRegexSnafu {
                    regex: with_anchors,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let new_add = defaults
            .add
            .iter()
            .filter(|arg| !self.remove.contains(*arg))
            .filter(|arg| !regexes.iter().any(|regex| regex.is_match(arg)))
            .chain(self.add.iter())
            .cloned()
            .collect();

        self.add = new_add;
        self.remove = HashSet::new();
        self.remove_regex = Vec::new();

        Ok(())
    }
}

/// This struct represents a role - e.g. HDFS datanodes or Trino workers. It has a key-value-map containing
/// all the roleGroups that are part of this role. Additionally, there is a `config`, which is configurable
/// at the role *and* roleGroup level. Everything at roleGroup level is merged on top of what is configured
/// on role level. There is also a second form of config, which can only be configured
/// at role level, the `roleConfig`.
/// You can learn more about this in the
/// [Roles and role group concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/roles-and-role-groups).
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
pub struct Role<
    T,
    U = GenericRoleConfig,
    ProductSpecificCommonConfig = GenericProductSpecificCommonConfig,
> where
    // Don't remove this trait bounds!!!
    // We don't know why, but if you remove either of them, the generated default value in the CRDs will
    // be missing!
    U: Default + JsonSchema + Serialize,
    ProductSpecificCommonConfig: Default + JsonSchema + Serialize,
{
    #[serde(
        flatten,
        bound(
            deserialize = "T: Default + Deserialize<'de>, ProductSpecificCommonConfig: Deserialize<'de>"
        )
    )]
    pub config: CommonConfiguration<T, ProductSpecificCommonConfig>,

    #[serde(default)]
    pub role_config: U,

    pub role_groups: HashMap<String, RoleGroup<T, ProductSpecificCommonConfig>>,
}

impl<T, U, ProductSpecificCommonConfig> Role<T, U, ProductSpecificCommonConfig>
where
    T: Configuration + 'static,
    U: Default + JsonSchema + Serialize,
    ProductSpecificCommonConfig: Default + JsonSchema + Serialize + Clone,
{
    /// This casts a generic struct implementing [`crate::product_config_utils::Configuration`]
    /// and used in [`Role`] into a Box of a dynamically dispatched
    /// [`crate::product_config_utils::Configuration`] Trait. This is required to use the generic
    /// [`Role`] with more than a single generic struct. For example different roles most likely
    /// have different structs implementing Configuration.
    pub fn erase(
        self,
    ) -> Role<Box<dyn Configuration<Configurable = T::Configurable>>, U, ProductSpecificCommonConfig>
    {
        Role {
            config: CommonConfiguration {
                config: Box::new(self.config.config)
                    as Box<dyn Configuration<Configurable = T::Configurable>>,
                config_overrides: self.config.config_overrides,
                env_overrides: self.config.env_overrides,
                cli_overrides: self.config.cli_overrides,
                pod_overrides: self.config.pod_overrides,
                product_specific_common_config: self.config.product_specific_common_config,
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
                                product_specific_common_config: group
                                    .config
                                    .product_specific_common_config,
                            },
                            replicas: group.replicas,
                        },
                    )
                })
                .collect(),
        }
    }
}

impl<T, U> Role<T, U, JavaCommonConfig>
where
    U: Default + JsonSchema + Serialize,
{
    /// Merges jvm argument overrides from
    ///
    /// 1. It takes the operator generated JVM args
    /// 2. It applies role level overrides
    /// 3. It applies roleGroup level overrides
    pub fn get_merged_jvm_argument_overrides(
        &self,
        role_group: &str,
        operator_generated: &JvmArgumentOverrides,
    ) -> Result<JvmArgumentOverrides, Error> {
        let from_role = &self
            .config
            .product_specific_common_config
            .jvm_argument_overrides;
        let from_role_group = &self
            .role_groups
            .get(role_group)
            .with_context(|| MissingRoleGroupSnafu { role_group })?
            .config
            .product_specific_common_config
            .jvm_argument_overrides;

        // Please note that the merge order is different than we normally do!
        // This is not trivial, as the merge operation is not purely additive (as it is with e.g. `PodTemplateSpec).
        let mut from_role = from_role.clone();
        from_role.try_merge(operator_generated)?;
        let mut from_role_group = from_role_group.clone();
        from_role_group.try_merge(&from_role)?;

        Ok(from_role_group)
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
    bound(
        deserialize = "T: Default + Deserialize<'de>, ProductSpecificCommonConfig: Default + Deserialize<'de>"
    )
)]
pub struct RoleGroup<T, ProductSpecificCommonConfig> {
    #[serde(flatten)]
    pub config: CommonConfiguration<T, ProductSpecificCommonConfig>,
    pub replicas: Option<u16>,
}

impl<T, ProductSpecificCommonConfig> RoleGroup<T, ProductSpecificCommonConfig> {
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
#[derive(Educe)]
#[educe(Clone, Debug)]
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::role_utils::JavaCommonConfig;

    #[test]
    fn test_merge_java_common_config() {
        // The operator generates some JVM arguments
        let operator_generated = JvmArgumentOverrides::new_with_only_additions(
            [
                "-Xms34406m".to_owned(),
                "-Xmx34406m".to_owned(),
                "-XX:+UseG1GC".to_owned(),
                "-XX:+ExitOnOutOfMemoryError".to_owned(),
                "-Djava.protocol.handler.pkgs=sun.net.www.protocol".to_owned(),
                "-Dsun.net.http.allowRestrictedHeaders=true".to_owned(),
                "-Djava.security.properties=/stackable/nifi/conf/security.properties".to_owned(),
            ]
            .into(),
        );

        let entire_role: Role<(), GenericRoleConfig, JavaCommonConfig> =
            serde_yaml::from_str("
                # Let's say we want to set some additional HTTP Proxy and IPv4 settings
                # And we don't like the garbage collector for some reason...
                jvmArgumentOverrides:
                  remove:
                    - -XX:+UseG1GC
                  add: # Add some networking arguments
                    - -Dhttps.proxyHost=proxy.my.corp
                    - -Dhttps.proxyPort=8080
                    - -Djava.net.preferIPv4Stack=true
                roleGroups:
                  default:
                    # For the roleGroup, let's say we need a different memory config.
                    # For that to work we first remove the flags generated by the operator and add our own.
                    # Also we override the proxy port to test that the roleGroup config takes precedence over the role config.
                    jvmArgumentOverrides:
                      removeRegex:
                        - -Xmx.*
                        - -Dhttps.proxyPort=.*
                      add:
                        - -Xmx40000m
                        - -Dhttps.proxyPort=1234
            ")
            .expect("Failed to parse role");

        let merged_jvm_argument_overrides = entire_role
            .get_merged_jvm_argument_overrides("default", &operator_generated)
            .expect("Failed to merge jvm argument overrides");

        let expected = Vec::from([
            "-Xms34406m".to_owned(),
            "-XX:+ExitOnOutOfMemoryError".to_owned(),
            "-Djava.protocol.handler.pkgs=sun.net.www.protocol".to_owned(),
            "-Dsun.net.http.allowRestrictedHeaders=true".to_owned(),
            "-Djava.security.properties=/stackable/nifi/conf/security.properties".to_owned(),
            "-Dhttps.proxyHost=proxy.my.corp".to_owned(),
            "-Djava.net.preferIPv4Stack=true".to_owned(),
            "-Xmx40000m".to_owned(),
            "-Dhttps.proxyPort=1234".to_owned(),
        ]);

        assert_eq!(
            merged_jvm_argument_overrides,
            JvmArgumentOverrides {
                add: expected.clone(),
                remove: HashSet::new(),
                remove_regex: Vec::new()
            }
        );

        assert_eq!(
            merged_jvm_argument_overrides.effective_jvm_config_after_merging(),
            &expected
        );
    }

    #[test]
    fn test_merge_java_common_config_keep_order() {
        let operator_generated =
            JvmArgumentOverrides::new_with_only_additions(["-Xms1m".to_owned()].into());

        let entire_role: Role<(), GenericRoleConfig, JavaCommonConfig> = serde_yaml::from_str(
            "
                jvmArgumentOverrides:
                  add:
                    - -Xms2m
                roleGroups:
                  default:
                    jvmArgumentOverrides:
                      add:
                        - -Xms3m
            ",
        )
        .expect("Failed to parse role");

        let merged_jvm_argument_overrides = entire_role
            .get_merged_jvm_argument_overrides("default", &operator_generated)
            .expect("Failed to merge jvm argument overrides");

        assert_eq!(
            merged_jvm_argument_overrides.effective_jvm_config_after_merging(),
            &[
                "-Xms1m".to_owned(),
                "-Xms2m".to_owned(),
                "-Xms3m".to_owned()
            ]
        );
    }
}
