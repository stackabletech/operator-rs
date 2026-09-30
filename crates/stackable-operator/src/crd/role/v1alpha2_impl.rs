use schemars::JsonSchema;
use serde::Serialize;
use snafu::OptionExt as _;

use crate::{
    config::{
        fragment::{self, FromFragment},
        merge::Merge,
    },
    crd::role::{
        java::{Error, JavaCommonConfig, JvmArgumentOverrides, MissingRoleGroupSnafu},
        v1alpha2::{Role, RoleGroup},
    },
};

impl<Config, ConfigOverrides, RoleConfig>
    Role<Config, ConfigOverrides, RoleConfig, JavaCommonConfig>
where
    RoleConfig: Default + JsonSchema + Serialize,
    ConfigOverrides: Default + JsonSchema + Serialize,
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

impl<Config, CommonConfig, ConfigOverrides> RoleGroup<Config, CommonConfig, ConfigOverrides> {
    pub fn validate_config<C, RoleConfig>(
        &self,
        role: &Role<Config, ConfigOverrides, RoleConfig, CommonConfig>,
        default_config: &Config,
    ) -> Result<C, fragment::ValidationError>
    where
        C: FromFragment<Fragment = Config>,
        Config: Merge + Clone,
        RoleConfig: Default + JsonSchema + Serialize,
        CommonConfig: Default + JsonSchema + Serialize,
        ConfigOverrides: Default + JsonSchema + Serialize,
    {
        let mut role_config = role.config.config.clone();
        role_config.merge(default_config);
        let mut rolegroup_config = self.config.config.clone();
        rolegroup_config.merge(&role_config);
        fragment::validate(rolegroup_config)
    }
}
