use crate::error::{Error, OperatorResult};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use schemars::gen::SchemaGenerator;
use schemars::schema::Schema;
use serde_json::{from_value, json};

/// Takes a [`LabelSelector`] and converts it to a String that can be used in Kubernetes API calls.
/// It will return an error if the LabelSelector contains illegal things (e.g. an `Exists` operator
/// with a value).
pub fn convert_label_selector_to_query_string(
    label_selector: &LabelSelector,
) -> OperatorResult<String> {
    let mut query_string = String::new();

    // match_labels are the "old" part of LabelSelectors.
    // They are the equivalent for the "In" operator in match_expressions
    // In a query string each key-value pair will be separated by an "=" and the pairs
    // are then joined on commas.
    // The whole match_labels part is optional so we only do this if there are match labels.
    if !label_selector.match_labels.is_empty() {
        query_string.push_str(
            &label_selector
                .match_labels
                .iter()
                .map(|(key, value)| format!("{}={}", key, value))
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    // Match expressions are more complex than match labels, both can appear in the same API call
    // They support these operators: "In", "NotIn", "Exists" and "DoesNotExist"

    // If we had match_labels AND we have match_expressions we need to separate those two
    // with a comma.
    if !label_selector.match_expressions.is_empty() {
        if !query_string.is_empty() {
            query_string.push(',');
        }

        // Here we map over all requirements (which might be empty) and for each of the requirements
        // we create a Result<String, Error> with the Ok variant being the converted match expression
        // We then collect those Results into a single Result with the Error being the _first_ error.
        // This, unfortunately means, that we'll throw away all but one error.
        // TODO: Return all errors in one go: https://github.com/stackabletech/operator-rs/issues/127
        let expression_string: Result<Vec<String>, Error> = label_selector
            .match_expressions
            .iter()
            .map(|requirement| match requirement.operator.as_str() {
                // In and NotIn can be handled the same, they both map to a simple "key OPERATOR (values)" string
                operator @ "In" | operator @ "NotIn" => match &requirement.values {
                    values if !values.is_empty() => Ok(format!(
                        "{} {} ({})",
                        requirement.key,
                        operator.to_ascii_lowercase(),
                        values.join(", ")
                    )),
                    _ => Err(Error::InvalidLabelSelector {
                        message: format!(
                            "LabelSelector has no or empty values for [{}] operator",
                            operator
                        ),
                    }),
                },
                // "Exists" is just the key and nothing else, if values have been specified it's an error
                "Exists" => match &requirement.values {
                    values if !values.is_empty() => Err(Error::InvalidLabelSelector {
                        message: "LabelSelector has [Exists] operator with values, this is not legal"
                            .to_string(),
                    }),
                    _ => Ok(requirement.key.to_string()),
                },
                // "DoesNotExist" is similar to "Exists" but it is preceded by an exclamation mark
                "DoesNotExist" => match &requirement.values {
                    values if !values.is_empty() => Err(Error::InvalidLabelSelector {
                        message:
                            "LabelSelector has [DoesNotExist] operator with values, this is not legal"
                                .to_string(),
                    }),
                    _ => Ok(format!("!{}", requirement.key)),
                },
                op => Err(Error::InvalidLabelSelector {
                    message: format!("LabelSelector has illegal/unknown operator [{}]", op),
                }),
            })
            .collect();

        query_string.push_str(&expression_string?.join(","));
    }

    Ok(query_string)
}

/// Returns a [`Schema`] that can be used with anything that has the same structure
/// as the `io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector` resource from Kubernetes.
///
/// This is needed because the [`LabelSelector`] from `k8s-openapi` does not derive `JsonSchema`.
///
/// # Example
///
/// ```
/// use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
/// use schemars::JsonSchema;
///
/// #[derive(JsonSchema)]
/// #[serde(rename_all = "camelCase")]
/// pub struct FooCrd {
///     #[schemars(schema_with = "stackable_operator::label_selector::schema")]
///     pub label_selector: LabelSelector,
/// }
/// ```
pub fn schema(_: &mut SchemaGenerator) -> Schema {
    from_value(json!({
      "description": "A label selector is a label query over a set of resources. The result of matchLabels and matchExpressions are ANDed. An empty label selector matches all objects. A null label selector matches no objects.",
      "properties": {
        "matchExpressions": {
          "description": "matchExpressions is a list of label selector requirements. The requirements are ANDed.",
          "items": {
            "description": "A label selector requirement is a selector that contains values, a key, and an operator that relates the key and values.",
            "properties": {
              "key": {
                "description": "key is the label key that the selector applies to.",
                "type": "string",
                "x-kubernetes-patch-merge-key": "key",
                "x-kubernetes-patch-strategy": "merge"
              },
              "operator": {
                "description": "operator represents a key's relationship to a set of values. Valid operators are In, NotIn, Exists and DoesNotExist.",
                "type": "string"
              },
              "values": {
                "description": "values is an array of string values. If the operator is In or NotIn, the values array must be non-empty. If the operator is Exists or DoesNotExist, the values array must be empty. This array is replaced during a strategic merge patch.",
                "items": {
                  "type": "string"
                },
                "type": "array"
              }
            },
            "required": [
              "key",
              "operator"
            ],
            "type": "object"
          },
          "type": "array"
        },
        "matchLabels": {
          "additionalProperties": {
            "type": "string"
          },
          "description": "matchLabels is a map of {key,value} pairs. A single {key,value} in the matchLabels map is equivalent to an element of matchExpressions, whose key field is \"key\", the operator is \"In\", and the values array contains only \"value\". The requirements are ANDed.",
          "type": "object"
        }
      },
      "type": "object"
    })).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement};
    use std::collections::BTreeMap;

    #[test]
    fn test_label_selector() {
        let mut match_labels = BTreeMap::new();
        match_labels.insert("foo".to_string(), "bar".to_string());
        match_labels.insert("hui".to_string(), "buh".to_string());

        let match_expressions = vec![
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "In".to_string(),
                values: vec!["bar".to_string()],
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "In".to_string(),
                values: vec!["quick".to_string(), "bar".to_string()],
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "NotIn".to_string(),
                values: vec!["quick".to_string(), "bar".to_string()],
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "Exists".to_string(),
                values: vec![],
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "DoesNotExist".to_string(),
                values: vec![],
            },
        ];

        let ls = LabelSelector {
            match_expressions: match_expressions,
            match_labels: match_labels.clone(),
        };
        assert_eq!(
            "foo=bar,hui=buh,foo in (bar),foo in (quick, bar),foo notin (quick, bar),foo,!foo",
            convert_label_selector_to_query_string(&ls).unwrap()
        );

        let ls = LabelSelector {
            match_expressions: vec![],
            match_labels,
        };
        assert_eq!(
            "foo=bar,hui=buh",
            convert_label_selector_to_query_string(&ls).unwrap()
        );

        let ls = LabelSelector {
            match_expressions: vec![],
            match_labels: BTreeMap::new(),
        };
        assert_eq!("", convert_label_selector_to_query_string(&ls).unwrap());
    }

    #[test]
    #[should_panic]
    fn test_invalid_label_in_selector() {
        let match_expressions = vec![LabelSelectorRequirement {
            key: "foo".to_string(),
            operator: "In".to_string(),
            values: vec![],
        }];

        let ls = LabelSelector {
            match_expressions,
            match_labels: BTreeMap::new(),
        };

        convert_label_selector_to_query_string(&ls).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_operator_in_selector() {
        let match_expressions = vec![LabelSelectorRequirement {
            key: "foo".to_string(),
            operator: "IllegalOperator".to_string(),
            values: vec![],
        }];

        let ls = LabelSelector {
            match_expressions,
            match_labels: BTreeMap::new(),
        };

        convert_label_selector_to_query_string(&ls).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_exists_operator_in_selector() {
        let match_expressions = vec![LabelSelectorRequirement {
            key: "foo".to_string(),
            operator: "Exists".to_string(),
            values: vec!["foobar".to_string()],
        }];

        let ls = LabelSelector {
            match_expressions,
            match_labels: BTreeMap::new(),
        };

        println!("{:?}", convert_label_selector_to_query_string(&ls).unwrap());
        convert_label_selector_to_query_string(&ls).unwrap();
    }
}
