use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;

use crate::{
    config::merge::Merge, k8s_openapi::DeepMerge, schemars, utils::crds::raw_object_schema,
};

// Variant of [`crate::config_overrides::KeyValueConfigOverrides`] that implements
// Merge
/// Flat key-value overrides for `*.properties`, Hadoop XML, etc.
///
/// This is backwards-compatible with the existing flat key-value YAML format
/// used by `HashMap<String, String>`.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, Merge, PartialEq, Serialize)]
#[merge(path_overrides(merge = "crate::config::merge"))]
pub struct KeyValueConfigOverrides {
    #[serde(flatten)]
    pub overrides: BTreeMap<String, Option<String>>,
}

// Variant of [`crate::config_overrides::JsonConfigOverrides`] with the following
// changes:
// - Implements Default
// - Implements Merge by using a Sequence variant which is not exposed in the CRD
// - `JsonPatches` was renamed to `JsonPatch` because it is one patch consisting of multiple
// operations.
// - `JsonPatch` contains a `json_patch::Patch` instead of a vector of strings
/// ConfigOverrides that can be applied to a JSON file.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonConfigOverrides {
    /// Can be set to arbitrary YAML content, which is converted to JSON and used as
    /// [RFC 7396](https://datatracker.ietf.org/doc/html/rfc7396) JSON merge patch.
    JsonMergePatch(serde_json::Value),

    /// An [RFC 6902](https://datatracker.ietf.org/doc/html/rfc6902) JSON patch.
    ///
    /// Can be used when more flexibility is needed, e.g. to only modify elements
    /// in a list based on a condition.
    ///
    /// A patch looks something like
    ///
    /// `- {"op": "test", "path": "/0/name", "value": "Andrew"}`
    ///
    /// or
    ///
    /// `- {"op": "add", "path": "/0/happy", "value": true}`
    JsonPatch(json_patch::Patch),

    /// Override the entire config file with the specified JSON value.
    UserProvided(serde_json::Value),

    /// Sequence of [`JsonConfigOverrides`] starting with the latest patch
    ///
    /// This variant is used internally to combine the role and role group configOverrides. They
    /// cannot be merged right away because the order of JsonPatch application affects the result.
    #[serde(skip)]
    Sequence(Vec<Self>),
}

impl JsonConfigOverrides {
    // Infallible variant of [`crate::config_overrides::JsonConfigOverrides::apply`]
    pub fn apply(&self, base: &serde_json::Value) -> serde_json::Value {
        match self {
            Self::JsonMergePatch(patch) => {
                let mut doc = base.clone();
                doc.merge_from(patch.clone());
                doc
            }
            Self::JsonPatch(patch) => {
                let mut doc = base.clone();
                if let Err(error) = json_patch::patch(&mut doc, patch) {
                    warn!("The JSON patch could not be applied: {error}");
                }
                doc
            }
            Self::UserProvided(content) => content.clone(),
            Self::Sequence(sequence) => {
                let mut doc = base.clone();
                // `sequence` starts with the latest patch. Iterate in reverse order, to apply the
                // patches from the first to the last one.
                for patch in sequence.iter().rev() {
                    doc = patch.apply(&doc);
                }
                doc
            }
        }
    }
}

impl Default for JsonConfigOverrides {
    fn default() -> Self {
        // There are several options to represent an empty patch, e.g.
        // `JsonConfigOverrides::Sequence(vec![])`. As this is exposed as the default in the CRD,
        // an empty JSON merge patch is returned, because JSON merge patches are the preferred way
        // to override the configuration.
        Self::JsonMergePatch(json!({}))
    }
}

impl Merge for JsonConfigOverrides {
    fn merge(&mut self, defaults: &Self) {
        let mut sequence = if let Self::Sequence(sequence) = self {
            sequence.clone()
        } else {
            vec![self.clone()]
        };

        if let Self::Sequence(base) = defaults {
            sequence.extend(base.clone());
        } else {
            sequence.push(defaults.clone());
        }

        *self = Self::Sequence(sequence);
    }
}

impl From<KeyValueConfigOverrides> for JsonConfigOverrides {
    fn from(value: KeyValueConfigOverrides) -> Self {
        Self::JsonMergePatch(value.overrides.into_iter().collect())
    }
}

/// ConfigOverrides as key-value pairs, JSON merge patch, JSON patch or JSON object.
///
/// The key-value pairs, JSON merge patch and JSON patch are merged with/applied to the
/// configuration provided by the operator. The user-provided JSON object replaces the
/// configuration of the operator.
///
/// Example for key-value pairs:
///
/// ```yaml
/// stringProperty: new value
/// booleanProperty: "true"
/// ```
///
/// Example for a JSON merge patch:
///
/// ```yaml
/// jsonMergePatch:
///   stringProperty: new value
///   booleanProperty: true
///   nestedProperty:
///     key: value
/// ```
///
/// Example for a JSON patch:
///
/// ```yaml
/// jsonPatch:
///   - op: replace
///     path: /stringProperty
///     value: new value
/// ```
///
/// Example for a JSON object:
///
/// ```yaml
/// userProvided:
///   stringProperty: new value
///   booleanProperty: true
///   nestedProperty:
///     key: value
/// ```
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
#[schemars(schema_with = "raw_object_schema")]
pub enum JsonOrKeyValueConfigOverrides {
    Json(JsonConfigOverrides),
    KeyValue(KeyValueConfigOverrides),
}

impl Default for JsonOrKeyValueConfigOverrides {
    fn default() -> Self {
        Self::Json(JsonConfigOverrides::default())
    }
}

impl From<JsonOrKeyValueConfigOverrides> for JsonConfigOverrides {
    fn from(value: JsonOrKeyValueConfigOverrides) -> Self {
        match value {
            JsonOrKeyValueConfigOverrides::KeyValue(key_value_config_overrides) => {
                key_value_config_overrides.into()
            }
            JsonOrKeyValueConfigOverrides::Json(json_config_overrides) => json_config_overrides,
        }
    }
}

impl Merge for JsonOrKeyValueConfigOverrides {
    fn merge(&mut self, defaults: &Self) {
        let mut self_json_config_overrides: JsonConfigOverrides = self.clone().into();
        let defaults_json_config_overrides = defaults.clone().into();

        self_json_config_overrides.merge(&defaults_json_config_overrides);

        *self = Self::Json(self_json_config_overrides);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::config::merge;

    #[test]
    fn test_json_config_overrides_apply() {
        let base = json!({
            "keyA": "base A",
            "keyB": "base B"
        });

        let json_merge_patch = JsonConfigOverrides::JsonMergePatch(json!({
            "keyB": "patch B",
            "keyC": "patch C"
        }));

        assert_eq!(
            json!({
                "keyA": "base A",
                "keyB": "patch B",
                "keyC": "patch C"
            }),
            json_merge_patch.apply(&base)
        );

        let json_patch = JsonConfigOverrides::JsonPatch(
            serde_json::from_value(json!([
              { "op": "replace", "path": "/keyB", "value": "patch B" },
              { "op": "add", "path": "/keyC", "value": "patch C" },
            ]))
            .expect("should contain valid JSON patch operations"),
        );

        assert_eq!(
            json!({
                "keyA": "base A",
                "keyB": "patch B",
                "keyC": "patch C",
            }),
            json_patch.apply(&base)
        );

        let invalid_json_patch = JsonConfigOverrides::JsonPatch(
            serde_json::from_value(json!([
              { "op": "replace", "path": "/keyB", "value": "patch B" },
              { "op": "remove", "path": "/keyD"  }
            ]))
            .expect("should contain valid JSON patch operations"),
        );

        // invalid_json_patch cannot be applied because the path "/keyD" does not exist in base.
        // A warning should be logged and the changes should be rolled back, i.e. "keyB" should be
        // "base B" instead of "patch B".
        assert_eq!(
            json!({
                "keyA": "base A",
                "keyB": "base B",
            }),
            invalid_json_patch.apply(&base)
        );

        let user_provided = JsonConfigOverrides::UserProvided(json!({
            "keyB": "patch B",
            "keyC": "patch C"
        }));

        assert_eq!(
            json!({
                "keyB": "patch B",
                "keyC": "patch C"
            }),
            user_provided.apply(&base)
        );

        let sequence = JsonConfigOverrides::Sequence(vec![
            // There should be no nested sequences, but as it is not technically prevented, it is
            // tested nevertheless.
            JsonConfigOverrides::Sequence(vec![JsonConfigOverrides::JsonMergePatch(json!({
                "keyC": "patch C.2",
                "keyD": "patch D.2"
            }))]),
            JsonConfigOverrides::JsonMergePatch(json!({
                "keyB": "patch B.1",
                "keyC": "patch C.1"
            })),
        ]);

        assert_eq!(
            json!({
                "keyA": "base A",
                "keyB": "patch B.1",
                "keyC": "patch C.2",
                "keyD": "patch D.2"
            }),
            sequence.apply(&base)
        );
    }

    #[test]
    fn test_json_config_overrides_merge() {
        let sequence1 = JsonConfigOverrides::Sequence(vec![
            JsonConfigOverrides::JsonMergePatch(json!({
                "key": "sequence 1.2",
            })),
            JsonConfigOverrides::JsonMergePatch(json!({
                "key": "sequence 1.1",
            })),
        ]);

        let sequence2 = JsonConfigOverrides::Sequence(vec![
            JsonConfigOverrides::JsonMergePatch(json!({
                "key": "sequence 2.2",
            })),
            JsonConfigOverrides::JsonMergePatch(json!({
                "key": "sequence 2.1",
            })),
        ]);

        // It does not matter for the test case if the JsonMergePatch, JsonPatch or UserProvided
        // variant is chosen.
        let json_merge_patch1 = JsonConfigOverrides::JsonMergePatch(json!({
            "key": "patch 1",
        }));

        let json_merge_patch2 = JsonConfigOverrides::JsonMergePatch(json!({
            "key": "patch 2",
        }));

        assert_eq!(
            JsonConfigOverrides::Sequence(vec![
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 2.2",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 2.1",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 1.2",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 1.1",
                })),
            ]),
            merge::merge(sequence2.clone(), &sequence1)
        );

        assert_eq!(
            JsonConfigOverrides::Sequence(vec![
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "patch 2",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 1.2",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 1.1",
                })),
            ]),
            merge::merge(json_merge_patch2.clone(), &sequence1)
        );

        assert_eq!(
            JsonConfigOverrides::Sequence(vec![
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 2.2",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "sequence 2.1",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "patch 1",
                })),
            ]),
            merge::merge(sequence2.clone(), &json_merge_patch1)
        );

        assert_eq!(
            JsonConfigOverrides::Sequence(vec![
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "patch 2",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "patch 1",
                })),
            ]),
            merge::merge(json_merge_patch2.clone(), &json_merge_patch1)
        );
    }

    #[test]
    fn test_json_config_overrides_from_key_value_config_overrides() {
        let key_value_config_overrides = KeyValueConfigOverrides {
            overrides: [("key".to_owned(), Some("value".to_owned()))].into(),
        };

        let actual_json_config_overrides: JsonConfigOverrides = key_value_config_overrides.into();

        let expected_json_config_overrides =
            JsonConfigOverrides::JsonMergePatch(json!({"key": "value"}));

        assert_eq!(expected_json_config_overrides, actual_json_config_overrides);
    }

    #[test]
    fn test_json_config_overrides_from_json_or_key_value_config_overrides() {
        let key_value_config_overrides =
            JsonOrKeyValueConfigOverrides::KeyValue(KeyValueConfigOverrides {
                overrides: [("key".to_owned(), Some("value".to_owned()))].into(),
            });

        let actual_json_config_overrides: JsonConfigOverrides = key_value_config_overrides.into();

        let expected_json_config_overrides =
            JsonConfigOverrides::JsonMergePatch(json!({"key": "value"}));

        assert_eq!(expected_json_config_overrides, actual_json_config_overrides);
    }

    #[test]
    fn test_json_or_key_value_config_overrides_merge() {
        let base = JsonOrKeyValueConfigOverrides::KeyValue(KeyValueConfigOverrides {
            overrides: [("key".to_owned(), Some("base".to_owned()))].into(),
        });

        let patch = JsonOrKeyValueConfigOverrides::KeyValue(KeyValueConfigOverrides {
            overrides: [("key".to_owned(), Some("patch".to_owned()))].into(),
        });

        // The merge implementation internally converts KeyValueConfigOverrides to
        // JsonConfigOverrides. It is already tested in [`test_json_config_overrides_merge`] that
        // merging JsonConfigOverrides works. Therefore, one test case with KeyValueConfigOverrides
        // is sufficient.
        assert_eq!(
            JsonOrKeyValueConfigOverrides::Json(JsonConfigOverrides::Sequence(vec![
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "patch",
                })),
                JsonConfigOverrides::JsonMergePatch(json!({
                    "key": "base",
                }))
            ])),
            merge::merge(patch, &base)
        );
    }
}
