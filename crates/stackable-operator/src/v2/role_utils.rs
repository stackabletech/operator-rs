use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use super::{
    builder::pod::container::EnvVarSet,
    jvm_argument_overrides::JvmArgumentOverrides,
    types::{
        kubernetes::{ClusterRoleName, RoleBindingName, ServiceAccountName},
        operator::{ClusterName, ProductName},
    },
};
use crate::{
    config::{
        fragment::{self, FromFragment},
        merge::{self, Merge, merge},
    },
    k8s_openapi::{DeepMerge, api::core::v1::PodTemplateSpec},
    role_utils::{CommonConfiguration, Role, RoleGroup},
    schemars::{self, JsonSchema},
};

// Variant of [`crate::role_utils::GenericCommonConfig`] that implements [`Merge`]
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Eq, Merge, PartialEq, Serialize)]
#[merge(path_overrides(merge = "crate::config::merge"))]
pub struct GenericCommonConfig {}

// Variant of [`crate::role_utils::JavaCommonConfig`] that implements [`Merge`]
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Merge, PartialEq, Eq, Serialize)]
#[merge(path_overrides(merge = "crate::config::merge"))]
#[serde(rename_all = "camelCase")]
pub struct JavaCommonConfig {
    /// Allows overriding JVM arguments.
    //
    /// Please read on the [JVM argument overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#jvm-argument-overrides)
    /// for details on the usage.
    #[serde(default)]
    pub jvm_argument_overrides: JvmArgumentOverrides,
}

/// Variant of [`crate::role_utils::RoleGroup`] that is easier to work with
///
/// Differences are:
/// * `config` is flattened.
/// * The [`HashMap`] in `env_overrides` is replaced with an [`EnvVarSet`].
#[derive(Clone, Debug, PartialEq)]
pub struct RoleGroupConfig<Config, CommonConfig, ConfigOverrides> {
    pub replicas: Option<u16>,
    pub config: Config,
    pub config_overrides: ConfigOverrides,
    pub env_overrides: EnvVarSet,
    pub cli_overrides: BTreeMap<String, String>,
    pub pod_overrides: PodTemplateSpec,
    pub product_specific_common_config: CommonConfig,
}

impl<Config, CommonConfig, ConfigOverrides> RoleGroupConfig<Config, CommonConfig, ConfigOverrides> {
    pub fn cli_overrides_to_vec(&self) -> Vec<String> {
        self.cli_overrides
            .clone()
            .into_iter()
            .flat_map(|(option, value)| [option, value])
            .collect()
    }
}

/// Merges and validates the [`RoleGroup`] with the given `role` and `default_config`
pub fn with_validated_config<ValidatedConfig, CommonConfig, Config, RoleConfig, ConfigOverrides>(
    role_group: &RoleGroup<Config, CommonConfig, ConfigOverrides>,
    role: &Role<Config, ConfigOverrides, RoleConfig, CommonConfig>,
    default_config: &Config,
) -> Result<RoleGroup<ValidatedConfig, CommonConfig, ConfigOverrides>, fragment::ValidationError>
where
    ValidatedConfig: FromFragment<Fragment = Config>,
    CommonConfig: Clone + Default + JsonSchema + Merge + Serialize,
    Config: Clone + Merge,
    RoleConfig: Default + JsonSchema + Serialize,
    ConfigOverrides: Clone + Default + JsonSchema + Merge + Serialize,
{
    let validated_config = role_group.validate_config(role, default_config)?;
    Ok(RoleGroup {
        config: CommonConfiguration {
            config: validated_config,
            config_overrides: merged_config_overrides(
                &role.config.config_overrides,
                role_group.config.config_overrides.clone(),
            ),
            env_overrides: merged_env_overrides(
                role.config.env_overrides.clone(),
                role_group.config.env_overrides.clone(),
            ),
            cli_overrides: merged_cli_overrides(
                role.config.cli_overrides.clone(),
                role_group.config.cli_overrides.clone(),
            ),
            pod_overrides: merged_pod_overrides(
                role.config.pod_overrides.clone(),
                role_group.config.pod_overrides.clone(),
            ),
            product_specific_common_config: merged_product_specific_common_config(
                &role.config.product_specific_common_config,
                role_group.config.product_specific_common_config.clone(),
            ),
        },
        replicas: role_group.replicas,
    })
}

fn merged_config_overrides<ConfigOverrides>(
    role_config_overrides: &ConfigOverrides,
    role_group_config_overrides: ConfigOverrides,
) -> ConfigOverrides
where
    ConfigOverrides: Merge,
{
    merge::merge(role_group_config_overrides, role_config_overrides)
}

fn merged_env_overrides(
    role_env_overrides: HashMap<String, String>,
    role_group_env_overrides: HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged_env_overrides = role_env_overrides;
    merged_env_overrides.extend(role_group_env_overrides);
    merged_env_overrides
}

fn merged_cli_overrides(
    role_cli_overrides: BTreeMap<String, String>,
    role_group_cli_overrides: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut merged_cli_overrides = role_cli_overrides;
    merged_cli_overrides.extend(role_group_cli_overrides);
    merged_cli_overrides
}

fn merged_pod_overrides(
    role_pod_overrides: PodTemplateSpec,
    role_group_pod_overrides: PodTemplateSpec,
) -> PodTemplateSpec {
    let mut merged_pod_overrides = role_pod_overrides;
    merged_pod_overrides.merge_from(role_group_pod_overrides);
    merged_pod_overrides
}

fn merged_product_specific_common_config<T>(role_config: &T, role_group_config: T) -> T
where
    T: Merge,
{
    merge(role_group_config, role_config)
}

/// Type-safe names for role resources
pub struct ResourceNames {
    pub cluster_name: ClusterName,
    pub product_name: ProductName,
}

impl ResourceNames {
    pub fn service_account_name(&self) -> ServiceAccountName {
        const SUFFIX: &str = "-serviceaccount";

        // compile-time checks
        const _: () = assert!(
            ClusterName::MAX_LENGTH + SUFFIX.len() <= ServiceAccountName::MAX_LENGTH,
            "The string `<cluster_name>-serviceaccount` must not exceed the limit of ServiceAccount names."
        );
        let _ = ClusterName::IS_RFC_1123_SUBDOMAIN_NAME;

        ServiceAccountName::from_str(&format!("{}{SUFFIX}", self.cluster_name))
            .expect("should be a valid ServiceAccount name")
    }

    pub fn role_binding_name(&self) -> RoleBindingName {
        const SUFFIX: &str = "-rolebinding";

        // compile-time checks
        const _: () = assert!(
            ClusterName::MAX_LENGTH + SUFFIX.len() <= RoleBindingName::MAX_LENGTH,
            "The string `<cluster_name>-rolebinding` must not exceed the limit of RoleBinding names."
        );
        let _ = ClusterName::IS_RFC_1123_SUBDOMAIN_NAME;

        RoleBindingName::from_str(&format!("{}{SUFFIX}", self.cluster_name))
            .expect("should be a valid RoleBinding name")
    }

    pub fn cluster_role_name(&self) -> ClusterRoleName {
        const SUFFIX: &str = "-clusterrole";

        // compile-time checks
        const _: () = assert!(
            ProductName::MAX_LENGTH + SUFFIX.len() <= ClusterRoleName::MAX_LENGTH,
            "The string `<cluster_name>-clusterrole` must not exceed the limit of cluster role names."
        );
        let _ = ProductName::IS_RFC_1123_SUBDOMAIN_NAME;

        ClusterRoleName::from_str(&format!("{}{SUFFIX}", self.product_name))
            .expect("should be a valid cluster role name")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use rstest::*;
    use serde::Serialize;

    use super::ResourceNames;
    use crate::{
        config::{fragment::Fragment, merge::Merge},
        k8s_openapi::api::core::v1::PodTemplateSpec,
        kube::api::ObjectMeta,
        role_utils::{CommonConfiguration, GenericRoleConfig, Role, RoleGroup},
        schemars::{self, JsonSchema},
        v2::{
            config_overrides::KeyValueConfigOverrides,
            role_utils::with_validated_config,
            types::{
                kubernetes::{ClusterRoleName, RoleBindingName, ServiceAccountName},
                operator::{ClusterName, ProductName},
            },
        },
    };

    #[derive(Debug, Fragment, PartialEq)]
    #[fragment(path_overrides(fragment = "crate::config::fragment"))]
    #[fragment_attrs(
        derive(Clone, Debug, Default, Merge, Eq, PartialEq),
        merge(path_overrides(merge = "crate::config::merge"))
    )]
    struct Config {
        property: String,
    }

    impl Config {
        fn new(value: &str) -> Self {
            Self {
                property: value.to_owned(),
            }
        }
    }

    impl ConfigFragment {
        fn new(value: Option<&str>) -> Self {
            Self {
                property: value.map(str::to_owned),
            }
        }
    }

    #[derive(Clone, Debug, Default, JsonSchema, Merge, PartialEq, Serialize)]
    #[merge(path_overrides(merge = "crate::config::merge"))]
    struct CommonConfig {
        property: Option<String>,
    }

    fn new_common_config<Config>(
        config: Config,
        override_value: Option<&str>,
    ) -> CommonConfiguration<Config, CommonConfig, KeyValueConfigOverrides> {
        let mut config_file_overrides = BTreeMap::new();
        let mut env_overrides = HashMap::new();
        let mut cli_overrides = BTreeMap::new();

        if let Some(value) = override_value {
            config_file_overrides.insert("property".to_owned(), value.to_owned());
            env_overrides.insert("PROPERTY".to_owned(), value.to_owned());
            cli_overrides.insert("--property".to_owned(), value.to_owned());
        }

        CommonConfiguration {
            config,
            config_overrides: KeyValueConfigOverrides {
                overrides: config_file_overrides,
            },
            env_overrides,
            cli_overrides,
            pod_overrides: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    name: override_value.map(str::to_owned),
                    ..ObjectMeta::default()
                }),
                ..PodTemplateSpec::default()
            },
            product_specific_common_config: CommonConfig {
                property: override_value.map(str::to_owned),
            },
        }
    }

    #[rstest]
    #[case(
        "role-group",
        Some("role-group"),
        Some("role-group"),
        Some("role"),
        Some("default")
    )]
    #[case(
        "role-group",
        Some("role-group"),
        Some("role-group"),
        Some("role"),
        None
    )]
    #[case(
        "role-group",
        Some("role-group"),
        Some("role-group"),
        None,
        Some("default")
    )]
    #[case("role-group", Some("role-group"), Some("role-group"), None, None)]
    #[case("role", Some("role"), None, Some("role"), Some("default"))]
    #[case("role", Some("role"), None, Some("role"), None)]
    #[case("default", None, None, None, Some("default"))]
    fn test_with_validated_config_and_result_ok(
        #[case] expected_config_value: &str,
        #[case] expected_override_value: Option<&str>,
        #[case] role_group_value: Option<&str>,
        #[case] role_value: Option<&str>,
        #[case] default_value: Option<&str>,
    ) {
        let role_group = RoleGroup {
            config: new_common_config(ConfigFragment::new(role_group_value), role_group_value),
            replicas: Some(3),
        };
        let role = Role::<_, _, GenericRoleConfig, _> {
            config: new_common_config(ConfigFragment::new(role_value), role_value),
            ..Role::default()
        };
        let default_config = ConfigFragment::new(default_value);

        let result = with_validated_config(&role_group, &role, &default_config);

        assert_eq!(
            Some(RoleGroup {
                config: new_common_config(
                    Config::new(expected_config_value),
                    expected_override_value
                ),
                replicas: Some(3)
            }),
            result.ok()
        );
    }

    #[test]
    fn test_with_validated_config_and_result_err() {
        let role_group = RoleGroup {
            config: new_common_config(ConfigFragment::new(None), None),
            replicas: None,
        };
        let role = Role::<_, _, GenericRoleConfig, _> {
            config: new_common_config(ConfigFragment::new(None), None),
            ..Role::default()
        };
        let default_config = ConfigFragment::new(None);

        let result: Result<RoleGroup<Config, _, _>, _> =
            with_validated_config(&role_group, &role, &default_config);

        assert!(result.is_err());
    }

    #[test]
    fn test_resource_names() {
        let resource_names = ResourceNames {
            cluster_name: ClusterName::from_str_unsafe("my-cluster"),
            product_name: ProductName::from_str_unsafe("my-product"),
        };

        assert_eq!(
            ServiceAccountName::from_str_unsafe("my-cluster-serviceaccount"),
            resource_names.service_account_name()
        );
        assert_eq!(
            RoleBindingName::from_str_unsafe("my-cluster-rolebinding"),
            resource_names.role_binding_name()
        );
        assert_eq!(
            ClusterRoleName::from_str_unsafe("my-product-clusterrole"),
            resource_names.cluster_role_name()
        );
    }
}
