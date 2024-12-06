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
    utils::crds::raw_object_schema,
};
use educe::Educe;
use k8s_openapi::api::core::v1::PodTemplateSpec;
use kube::{runtime::reflector::ObjectRef, Resource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("missing roleGroup {role_group:?}"))]
    MissingRoleGroup { role_group: String },
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
    // It is not accessible by operators, please use <TODO link to functions> to read the value
    #[serde(flatten, default)]
    pub(crate) product_specific_common_config: ProductSpecificCommonConfig,
}

impl<T, ProductSpecificCommonConfig> CommonConfiguration<T, ProductSpecificCommonConfig> {
    fn default_config() -> serde_json::Value {
        serde_json::json!({})
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct GenericProductSpecificCommonConfig {}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, Merge)]
#[merge(path_overrides(merge = "crate::config::merge"))]
#[serde(rename_all = "camelCase")]
pub struct JavaCommonConfig {
    /// Allows overriding JVM arguments.
    ///
    // TODO: Docs
    // Use [`JavaCommonConfig::effective_jvm_config`] to retrieve the effective JVM arguments!
    #[serde(default)]
    pub jvm_argument_overrides: BTreeMap<String, JvmArgument>,
}

impl JavaCommonConfig {
    /// Returns all arguments that should be passed to the JVM.
    ///
    /// Please note that the values of the [`BTreeMap`] are [`Option<String>`]. A value of [`None`]
    /// expresses that the given argument is just a flag without any argument.
    pub fn effective_jvm_config(&self) -> BTreeMap<String, Option<String>> {
        self.jvm_argument_overrides
            .iter()
            .filter_map(|(k, v)| match v {
                JvmArgument::Argument(argument) => Some((k.to_owned(), Some(argument.to_owned()))),
                JvmArgument::Flag {} => Some((k.to_owned(), None)),
                JvmArgument::Remove {} => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JvmArgument {
    Argument(String),
    Flag {},
    Remove {},
}
impl Merge for JvmArgument {
    fn merge(&mut self, _defaults: &Self) {
        // We ignore whatever was in there before, later values override earlier ones
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
    ProductSpecificCommonConfig: Default + JsonSchema + Serialize + Clone + Merge,
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

    pub fn merged_product_specific_common_config(
        &self,
        role_group: &str,
    ) -> Result<ProductSpecificCommonConfig, Error> {
        let from_role = &self.config.product_specific_common_config;
        let mut merged = self
            .role_groups
            .get(role_group)
            .with_context(|| MissingRoleGroupSnafu { role_group })?
            .config
            .product_specific_common_config
            .clone();
        merged.merge(from_role);

        Ok(merged)
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
    use std::collections::BTreeMap;

    use crate::{config::merge::Merge, role_utils::JvmArgument};

    use super::JavaCommonConfig;

    #[test]
    fn test_parse_java_common_config() {
        let input = r#"
            jvmArgumentOverrides:
              -XX:+UseG1GC:
                flag: {}
              -Dhttps.proxyHost:
                argument: proxy.my.corp
              -XX:+ExitOnOutOfMemoryError:
                remove: {}
        "#;
        let deserializer = serde_yaml::Deserializer::from_str(input);
        let java_common_config: JavaCommonConfig =
            serde_yaml::with::singleton_map_recursive::deserialize(deserializer).unwrap();

        assert_eq!(
            java_common_config,
            JavaCommonConfig {
                jvm_argument_overrides: BTreeMap::from([
                    ("-XX:+UseG1GC".to_owned(), JvmArgument::Flag {}),
                    (
                        "-Dhttps.proxyHost".to_owned(),
                        JvmArgument::Argument("proxy.my.corp".to_owned())
                    ),
                    (
                        "-XX:+ExitOnOutOfMemoryError".to_owned(),
                        JvmArgument::Remove {}
                    )
                ])
            }
        );
    }

    #[test]
    fn test_merge_java_common_config() {
        // The operator generates some JVM arguments
        let operator_generated = JavaCommonConfig {
            jvm_argument_overrides: BTreeMap::from([
                // Some flags
                ("-Xms34406m".to_owned(), JvmArgument::Flag {}),
                ("-Xmx34406m".to_owned(), JvmArgument::Flag {}),
                ("-XX:+UseG1GC".to_owned(), JvmArgument::Flag {}),
                (
                    "-XX:+ExitOnOutOfMemoryError".to_owned(),
                    JvmArgument::Flag {},
                ),
                // And some arguments
                (
                    "-Djava.protocol.handler.pkgs".to_owned(),
                    JvmArgument::Argument("sun.net.www.protocol".to_owned()),
                ),
                (
                    "-Dsun.net.http.allowRestrictedHeaders".to_owned(),
                    JvmArgument::Argument(true.to_string()),
                ),
                (
                    "-Djava.security.properties".to_owned(),
                    JvmArgument::Argument("/stackable/nifi/conf/security.properties".to_owned()),
                ),
            ]),
        };

        // Let's say we want to set some additional HTTP Proxy and IPv4 settings
        // And we don't like the garbage collector for some reason...
        let role = serde_yaml::Deserializer::from_str(
            r#"
            jvmArgumentOverrides:
              -XX:+UseG1GC:
                remove: {}
              -Dhttps.proxyHost:
                argument: proxy.my.corp
              -Dhttps.proxyPort:
                argument: "8080"
              -Djava.net.preferIPv4Stack:
                argument: "true"
            "#,
        );
        let role: JavaCommonConfig =
            serde_yaml::with::singleton_map_recursive::deserialize(role).unwrap();

        // For the roleGroup, let's say we need a different memory config.
        // For that to work we first remove the flags generated by the operator and add our own.
        // Also we override the proxy port to test that the roleGroup config takes precedence over the role config.
        let role_group = serde_yaml::Deserializer::from_str(
            r#"
            jvmArgumentOverrides:
              # We need more memory!
              -Xmx34406m:
                remove: {}
              -Xmx40000m:
                flag: {}
              -Dhttps.proxyPort:
                argument: "1234"
            "#,
        );
        let role_group: JavaCommonConfig =
            serde_yaml::with::singleton_map_recursive::deserialize(role_group).unwrap();

        let mut merged = role_group;
        merged.merge(&role);
        merged.merge(&operator_generated);

        assert_eq!(
            merged,
            JavaCommonConfig {
                jvm_argument_overrides: BTreeMap::from([
                    // Flags
                    ("-Xms34406m".to_owned(), JvmArgument::Flag {}),
                    // Note the different memory config from the roleGroup!
                    ("-Xmx34406m".to_owned(), JvmArgument::Remove {}),
                    ("-Xmx40000m".to_owned(), JvmArgument::Flag {}),
                    // Note that the "-XX:+UseG1GC" flag is removed!
                    ("-XX:+UseG1GC".to_owned(), JvmArgument::Remove {}),
                    (
                        "-XX:+ExitOnOutOfMemoryError".to_owned(),
                        JvmArgument::Flag {},
                    ),
                    // Arguments
                    (
                        "-Djava.protocol.handler.pkgs".to_owned(),
                        JvmArgument::Argument("sun.net.www.protocol".to_owned()),
                    ),
                    (
                        "-Dsun.net.http.allowRestrictedHeaders".to_owned(),
                        JvmArgument::Argument(true.to_string()),
                    ),
                    (
                        "-Djava.security.properties".to_owned(),
                        JvmArgument::Argument(
                            "/stackable/nifi/conf/security.properties".to_owned()
                        ),
                    ),
                    (
                        "-Dhttps.proxyHost".to_owned(),
                        JvmArgument::Argument("proxy.my.corp".to_owned()),
                    ),
                    (
                        "-Dhttps.proxyPort".to_owned(),
                        // Note: This is overridden by the roleGroup
                        JvmArgument::Argument("1234".to_owned()),
                    ),
                    (
                        "-Djava.net.preferIPv4Stack".to_owned(),
                        JvmArgument::Argument("true".to_owned()),
                    ),
                ])
            }
        );

        assert_eq!(
            merged.effective_jvm_config(),
            BTreeMap::from([
                ("-Xms34406m".to_owned(), None),
                ("-Xmx40000m".to_owned(), None),
                ("-XX:+ExitOnOutOfMemoryError".to_owned(), None),
                (
                    "-Djava.protocol.handler.pkgs".to_owned(),
                    Some("sun.net.www.protocol".to_owned())
                ),
                (
                    "-Dsun.net.http.allowRestrictedHeaders".to_owned(),
                    Some("true".to_owned())
                ),
                (
                    "-Djava.security.properties".to_owned(),
                    Some("/stackable/nifi/conf/security.properties".to_owned())
                ),
                (
                    "-Dhttps.proxyHost".to_owned(),
                    Some("proxy.my.corp".to_owned())
                ),
                ("-Dhttps.proxyPort".to_owned(), Some("1234".to_owned())),
                (
                    "-Djava.net.preferIPv4Stack".to_owned(),
                    Some("true".to_owned())
                ),
            ])
        );
    }
}
