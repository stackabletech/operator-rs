use std::collections::HashSet;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt as _, Snafu};

#[cfg(doc)]
use crate::config::merge::Merge;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("missing roleGroup {role_group:?}"))]
    MissingRoleGroup { role_group: String },

    #[snafu(display(
        "Could not parse regex from \"jvmArgumentOverrides.removeRegex\", ignoring it (there might be some added anchors at the start and end): {regex:?}"
    ))]
    InvalidRemoveRegex { source: regex::Error, regex: String },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaCommonConfig {
    /// Allows overriding JVM arguments.
    //
    /// Please read on the [JVM argument overrides documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/overrides#jvm-argument-overrides)
    /// for details on the usage.
    #[serde(default)]
    pub jvm_argument_overrides: JvmArgumentOverrides,
}

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

#[cfg(feature = "crds")]
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{
        crd::role::{GenericRoleConfig, v1alpha2::Role},
        test_utils::deserialize_from_yaml_with_singleton_map,
    };

    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct EmptyConfigOverrides {}

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

        let entire_role: Role<(), EmptyConfigOverrides, GenericRoleConfig, JavaCommonConfig> =
            deserialize_from_yaml_with_singleton_map("
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
                    # Replicas is required since v1alpha2 RoleGroup.
                    replicas:
                      fixed:
                        count: 1
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

        let entire_role: Role<(), EmptyConfigOverrides, GenericRoleConfig, JavaCommonConfig> =
            deserialize_from_yaml_with_singleton_map(
                "
                jvmArgumentOverrides:
                  add:
                    - -Xms2m
                roleGroups:
                  default:
                    # Replicas is required since v1alpha2 RoleGroup.
                    replicas:
                      fixed:
                        count: 1
                    jvmArgumentOverrides:
                      add:
                        - -Xms3m
            ",
            )
            .expect("role must parse");

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
