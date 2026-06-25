use std::{borrow::Cow, collections::HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize, Serializer, de::Error};

use crate::config::merge::Merge;

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
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
    remove_regex: RegexSet,

    /// Sequence of [`JvmArgumentOverrides`] which must be applied before this one
    ///
    /// This field is used internally to combine the role and role group overrides. The fields of
    /// the role group cannot just be appended to the ones of the role because the fields `remove`,
    /// `remove_regex` and `add` of the role must be applied before the ones of the role group.
    #[serde(skip)]
    preceding_overrides: Vec<Self>,
}

impl Merge for JvmArgumentOverrides {
    fn merge(&mut self, defaults: &Self) {
        self.preceding_overrides.push(defaults.clone());
    }
}

impl JvmArgumentOverrides {
    pub fn apply_to(&self, jvm_arguments: impl IntoIterator<Item = String>) -> Vec<String> {
        // 1. Apply the preceding overrides
        self.preceding_overrides
            .iter()
            // The vector should only contain one element, but if it contains more than one then
            // start with the one that was added last.
            .rev()
            .fold(
                jvm_arguments.into_iter().collect(),
                |jvm_arguments, overrides| overrides.apply_to(jvm_arguments),
            )
            .into_iter()
            // 2. Remove exact matches
            .filter(|arg| !self.remove.contains(arg))
            // 3. Remove arguments matching the regexes
            .filter(|arg| !self.remove_regex.0.is_match(arg))
            // 4. Add arguments
            .chain(self.add.clone())
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
struct RegexSet(regex::RegexSet);

impl<'de> Deserialize<'de> for RegexSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let regexes = <Vec<Cow<'de, str>>>::deserialize(deserializer)?;

        let anchored_regexes = regexes
            .iter()
            .map(|maybe_anchored_regex| {
                maybe_anchored_regex
                    .trim_start_matches('^')
                    .trim_end_matches('$')
            })
            .map(|unanchored_regex| format!("^{unanchored_regex}$"));

        match regex::RegexSet::new(anchored_regexes) {
            Ok(regexset) => Ok(Self(regexset)),
            Err(err) => Err(D::Error::custom(err)),
        }
    }
}

impl Serialize for RegexSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.patterns().serialize(serializer)
    }
}

impl Eq for RegexSet {}

impl PartialEq for RegexSet {
    fn eq(&self, other: &Self) -> bool {
        self.0.patterns() == other.0.patterns()
    }
}

impl JsonSchema for RegexSet {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RegexSet".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": {
                "type": "string"
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use stackable_operator_derive::Fragment;

    use super::*;
    use crate::{
        role_utils::{GenericRoleConfig, Role, RoleGroup},
        v2::role_utils::{JavaCommonConfig, with_validated_config},
    };

    // #[derive(
    //     Clone, Debug, Default, Deserialize, Fragment, JsonSchema, Merge, PartialEq, Serialize,
    // )]
    #[derive(Debug, Fragment, PartialEq)]
    #[fragment_attrs(derive(Clone, Debug, Default, Deserialize, Eq, PartialEq))]
    #[fragment(path_overrides(fragment = "crate::config::fragment",))]
    struct EmptyConfig {}

    impl Merge for EmptyConfigFragment {
        fn merge(&mut self, _defaults: &Self) {}
    }

    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, Merge, PartialEq, Serialize)]
    #[merge(path_overrides(merge = "crate::config::merge"))]
    struct EmptyConfigOverrides {}

    #[test]
    fn test_merge_java_common_config() {
        // The operator generates some JVM arguments
        let operator_generated = [
            "-Xms34406m".to_owned(),
            "-Xmx34406m".to_owned(),
            "-XX:+UseG1GC".to_owned(),
            "-XX:+ExitOnOutOfMemoryError".to_owned(),
            "-Djava.protocol.handler.pkgs=sun.net.www.protocol".to_owned(),
            "-Dsun.net.http.allowRestrictedHeaders=true".to_owned(),
            "-Djava.security.properties=/stackable/nifi/conf/security.properties".to_owned(),
        ];

        let entire_role: Role<
            EmptyConfigFragment,
            EmptyConfigOverrides,
            GenericRoleConfig,
            JavaCommonConfig,
        > = serde_yaml::from_str(
            "
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

        let role_group = entire_role
            .role_groups
            .get("default")
            .expect("role group should be defined");

        let validated_config: RoleGroup<EmptyConfig, _, _> =
            with_validated_config(role_group, &entire_role, &EmptyConfigFragment {})
                .expect("role spec should be valid");

        let effective_jvm_config = validated_config
            .config
            .product_specific_common_config
            .jvm_argument_overrides
            .apply_to(operator_generated);

        let expected = vec![
            "-Xms34406m".to_owned(),
            "-XX:+ExitOnOutOfMemoryError".to_owned(),
            "-Djava.protocol.handler.pkgs=sun.net.www.protocol".to_owned(),
            "-Dsun.net.http.allowRestrictedHeaders=true".to_owned(),
            "-Djava.security.properties=/stackable/nifi/conf/security.properties".to_owned(),
            "-Dhttps.proxyHost=proxy.my.corp".to_owned(),
            "-Djava.net.preferIPv4Stack=true".to_owned(),
            "-Xmx40000m".to_owned(),
            "-Dhttps.proxyPort=1234".to_owned(),
        ];

        assert_eq!(effective_jvm_config, expected);
    }

    #[test]
    fn test_merge_java_common_config_keep_order() {
        let operator_generated = ["-Xms1m".to_owned()];

        let entire_role: Role<
            EmptyConfigFragment,
            EmptyConfigOverrides,
            GenericRoleConfig,
            JavaCommonConfig,
        > = serde_yaml::from_str(
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

        let role_group = entire_role
            .role_groups
            .get("default")
            .expect("role group should be defined");

        let validated_config: RoleGroup<EmptyConfig, _, _> =
            with_validated_config(role_group, &entire_role, &EmptyConfigFragment {})
                .expect("role spec should be valid");

        let effective_jvm_config = validated_config
            .config
            .product_specific_common_config
            .jvm_argument_overrides
            .apply_to(operator_generated);

        assert_eq!(
            effective_jvm_config,
            &[
                "-Xms1m".to_owned(),
                "-Xms2m".to_owned(),
                "-Xms3m".to_owned()
            ]
        );
    }
}
