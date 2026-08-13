use std::collections::{BTreeMap, btree_map};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::v2::builder::pod::container::EnvVarName;

/// A map from environment variable names to their values.
///
/// This is a newtype around `BTreeMap<EnvVarName, String>` instead of a bare type alias because a
/// `BTreeMap` keyed by [`EnvVarName`] would generate a JSON schema using `patternProperties` (from
/// the [`EnvVarName`] pattern), which is not supported in CRDs. The custom [`JsonSchema`]
/// implementation therefore exposes the field as a plain `BTreeMap<String, String>` in the CRD.
///
/// As a consequence, the Kubernetes API server does not enforce the [`EnvVarName`] pattern:
/// invalid names are accepted on `apply` and only rejected later, when the operator deserializes
/// the resource.
///
/// This uses a `BTreeMap<EnvVarName, String>` rather than an
/// [`EnvVarSet`](crate::v2::builder::pod::container::EnvVarSet), because for overrides only plain
/// values are supported at the moment. An `EnvVarSet` maps each name to a full `EnvVar`, which also
/// allows the other variants (such as `valueFrom`); those are intentionally not exposed here.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvOverrides(BTreeMap<EnvVarName, String>);

impl EnvOverrides {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn insert(&mut self, env_var_name: EnvVarName, value: String) -> Option<String> {
        self.0.insert(env_var_name, value)
    }

    pub fn iter(&self) -> btree_map::Iter<'_, EnvVarName, String> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(EnvVarName, String)> for EnvOverrides {
    fn from_iter<T: IntoIterator<Item = (EnvVarName, String)>>(iter: T) -> Self {
        Self(BTreeMap::from_iter(iter))
    }
}

impl<'a> IntoIterator for &'a EnvOverrides {
    type IntoIter = btree_map::Iter<'a, EnvVarName, String>;
    type Item = (&'a EnvVarName, &'a String);

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl JsonSchema for EnvOverrides {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "EnvOverrides".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        BTreeMap::<String, String>::json_schema(generator)
    }
}

impl IntoIterator for EnvOverrides {
    type IntoIter = btree_map::IntoIter<EnvVarName, String>;
    type Item = (EnvVarName, String);

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Extend<(EnvVarName, String)> for EnvOverrides {
    fn extend<T: IntoIterator<Item = (EnvVarName, String)>>(&mut self, iter: T) {
        iter.into_iter().for_each(move |(k, v)| {
            self.0.insert(k, v);
        });
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deserialize_valid_names() {
        let overrides: EnvOverrides = serde_json::from_value(json!({
            "FOO": "1",
            "BAR": "2"
        }))
        .expect("should be valid EnvOverrides");

        assert_eq!(
            vec![
                (EnvVarName::from_str_unsafe("BAR"), "2".to_owned()),
                (EnvVarName::from_str_unsafe("FOO"), "1".to_owned())
            ],
            overrides.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn deserialize_rejects_invalid_names() {
        // "=" is not allowed in environment variable names.
        let result: Result<EnvOverrides, serde_json::Error> = serde_json::from_value(json!({
            "FO=O": "1"
        }));

        assert_eq!(
            Err(
                "no match for the regular expression \"^[ -<>-~]+$\" in the value \"FO=O\""
                    .to_owned()
            ),
            result.map_err(|err| err.to_string())
        );
    }

    #[test]
    fn json_schema_is_a_plain_string_map() {
        let schema = serde_json::to_value(schemars::schema_for!(EnvOverrides))
            .expect("should produce a valid JSON schema");

        assert_eq!(
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "EnvOverrides",
                "type": "object",
                "additionalProperties": {
                    "type": "string"
                }
            }),
            schema
        );
    }
}
