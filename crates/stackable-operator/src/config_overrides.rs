//! Building-block types for strategy-based `configOverrides`.
//!
//! Operators declare typed override structs choosing patch strategies per file
//! (e.g. [`JsonConfigOverrides`] for JSON files, [`KeyValueConfigOverrides`] for
//! properties files). The types here are composed by each operator into its
//! CRD-specific `configOverrides` struct.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::utils::crds::raw_object_schema;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to serialize base document to JSON"))]
    SerializeBaseDocument { source: serde_json::Error },

    #[snafu(display("failed to apply JSON patch (RFC 6902)"))]
    ApplyJsonPatch { source: json_patch::PatchError },

    #[snafu(display("failed to deserialize JSON patch operation {index} from string"))]
    DeserializeJsonPatchOperation {
        source: serde_json::Error,
        index: usize,
    },

    #[snafu(display("failed to parse user-provided JSON content"))]
    ParseUserProvidedJson { source: serde_json::Error },
}

/// Trait that allows the product config pipeline to extract flat key-value
/// overrides from any `configOverrides` type.
///
/// Typed override structs that have no key-value files can use the default
/// implementation, which returns an empty map.
pub trait KeyValueOverridesProvider {
    fn get_key_value_overrides(&self, _file: &str) -> BTreeMap<String, Option<String>> {
        BTreeMap::new()
    }
}

/// Flat key-value overrides for `*.properties`, Hadoop XML, etc.
///
/// This is backwards-compatible with the existing flat key-value YAML format
/// used by `HashMap<String, String>`.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct KeyValueConfigOverrides {
    #[serde(flatten)]
    pub overrides: BTreeMap<String, String>,
}

impl KeyValueConfigOverrides {
    /// Returns the overrides as a `BTreeMap<String, Option<String>>`, matching
    /// the format expected by the product config pipeline.
    ///
    /// This is useful when implementing [`KeyValueOverridesProvider`] for a
    /// typed override struct that contains [`KeyValueConfigOverrides`] fields.
    pub fn as_overrides(&self) -> BTreeMap<String, Option<String>> {
        self.overrides
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect()
    }
}

/// ConfigOverrides that can be applied to a JSON file.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonConfigOverrides {
    /// Can be set to arbitrary YAML content, which is converted to JSON and used as
    /// [RFC 7396](https://datatracker.ietf.org/doc/html/rfc7396) JSON merge patch.
    #[schemars(schema_with = "raw_object_schema")]
    JsonMergePatch(serde_json::Value),

    /// List of [RFC 6902](https://datatracker.ietf.org/doc/html/rfc6902) JSON patches.
    ///
    /// Can be used when more flexibility is needed, e.g. to only modify elements
    /// in a list based on a condition.
    ///
    /// A patch looks something like
    ///
    /// `{"op": "test", "path": "/0/name", "value": "Andrew"}`
    ///
    /// or
    ///
    /// `{"op": "add", "path": "/0/happy", "value": true}`
    JsonPatches(Vec<String>),

    /// Override the entire config file with the specified String.
    UserProvided(String),
}

impl JsonConfigOverrides {
    /// Applies this override to a base JSON document and returns the patched
    /// document as a [`serde_json::Value`].
    ///
    /// For [`JsonConfigOverrides::JsonMergePatch`] and
    /// [`JsonConfigOverrides::JsonPatches`], the base document is patched
    /// according to the respective RFC.
    ///
    /// For [`JsonConfigOverrides::UserProvided`], the base document is ignored
    /// entirely and the user-provided string is parsed and returned.
    pub fn apply(&self, base: &serde_json::Value) -> Result<serde_json::Value, Error> {
        match self {
            JsonConfigOverrides::JsonMergePatch(patch) => {
                let mut doc = base.clone();
                json_patch::merge(&mut doc, patch);
                Ok(doc)
            }
            JsonConfigOverrides::JsonPatches(patches) => {
                let mut doc = base.clone();
                let operations: Vec<json_patch::PatchOperation> = patches
                    .iter()
                    .enumerate()
                    .map(|(index, patch_str)| {
                        serde_json::from_str(patch_str)
                            .context(DeserializeJsonPatchOperationSnafu { index })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                json_patch::patch(&mut doc, &operations).context(ApplyJsonPatchSnafu)?;
                Ok(doc)
            }
            JsonConfigOverrides::UserProvided(content) => {
                serde_json::from_str(content).context(ParseUserProvidedJsonSnafu)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_merge_patch_add_and_overwrite_fields() {
        let base = json!({
            "bundles": {
                "authz": {
                    "polling": {
                        "min_delay_seconds": 10,
                        "max_delay_seconds": 20
                    }
                }
            }
        });

        let overrides = JsonConfigOverrides::JsonMergePatch(json!({
            "bundles": {
                "authz": {
                    "polling": {
                        "min_delay_seconds": 3,
                        "max_delay_seconds": 5
                    }
                }
            },
            "default_decision": "/http/example/authz/allow"
        }));

        let result = overrides.apply(&base).expect("merge patch should succeed");

        assert_eq!(
            result["bundles"]["authz"]["polling"]["min_delay_seconds"],
            3
        );
        assert_eq!(
            result["bundles"]["authz"]["polling"]["max_delay_seconds"],
            5
        );
        assert_eq!(result["default_decision"], "/http/example/authz/allow");
    }

    #[test]
    fn json_merge_patch_remove_field_with_null() {
        let base = json!({
            "keep": "this",
            "remove": "this"
        });

        let overrides = JsonConfigOverrides::JsonMergePatch(json!({
            "remove": null
        }));

        let result = overrides.apply(&base).expect("merge patch should succeed");

        assert_eq!(result["keep"], "this");
        assert!(result.get("remove").is_none());
    }

    #[test]
    fn json_patch_add_remove_replace() {
        let base = json!({
            "foo": "bar",
            "baz": "qux"
        });

        let overrides = JsonConfigOverrides::JsonPatches(vec![
            r#"{"op": "replace", "path": "/foo", "value": "replaced"}"#.to_owned(),
            r#"{"op": "remove", "path": "/baz"}"#.to_owned(),
            r#"{"op": "add", "path": "/new_key", "value": "new_value"}"#.to_owned(),
        ]);

        let result = overrides.apply(&base).expect("JSON patch should succeed");

        assert_eq!(result["foo"], "replaced");
        assert!(result.get("baz").is_none());
        assert_eq!(result["new_key"], "new_value");
    }

    #[test]
    fn json_patch_invalid_path_returns_error() {
        let base = json!({"foo": "bar"});

        let overrides = JsonConfigOverrides::JsonPatches(vec![
            r#"{"op": "remove", "path": "/nonexistent"}"#.to_owned(),
        ]);

        let result = overrides.apply(&base);
        assert!(
            matches!(result.unwrap_err(), Error::ApplyJsonPatch { source } if source.to_string()
                    == "operation '/0' failed at path '/nonexistent': path is invalid"),
            "removing a nonexistent path should fail"
        );
    }

    #[test]
    fn json_patch_invalid_operation_returns_error() {
        let base = json!({"foo": "bar"});

        let overrides = JsonConfigOverrides::JsonPatches(vec![r#"{"not_an_op": true}"#.to_owned()]);

        let result = overrides.apply(&base);
        assert!(
            matches!(result.unwrap_err(), Error::DeserializeJsonPatchOperation { source, index: 0 } if source.to_string()
                    == "missing field `op` at line 1 column 19"),
            "invalid patch operation should return an error"
        );
    }

    #[test]
    fn user_provided_ignores_base() {
        let base = json!({"foo": "bar"});
        let content = "{\"custom\": true}";

        let overrides = JsonConfigOverrides::UserProvided(content.to_owned());

        let result = overrides
            .apply(&base)
            .expect("user provided should succeed");
        assert_eq!(result, json!({"custom": true}));
    }

    #[test]
    fn user_provided_invalid_json_returns_error() {
        let base = json!({"foo": "bar"});

        let overrides = JsonConfigOverrides::UserProvided("not valid json".to_owned());

        let result = overrides.apply(&base);
        assert!(
            matches!(result.unwrap_err(), Error::ParseUserProvidedJson { source } if source.to_string()
                    == "expected ident at line 1 column 2"),
            "invalid JSON should return an error"
        );
    }

    #[test]
    fn key_value_config_overrides_as_overrides() {
        let mut overrides = BTreeMap::new();
        overrides.insert("key1".to_owned(), "value1".to_owned());
        overrides.insert("key2".to_owned(), "value2".to_owned());

        let kv = KeyValueConfigOverrides { overrides };
        let result = kv.as_overrides();

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("key1"), Some(&Some("value1".to_owned())));
        assert_eq!(result.get("key2"), Some(&Some("value2".to_owned())));
    }

}
