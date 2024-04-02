use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use snafu::Snafu;

type Result<T, E = SelectorError> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum SelectorError {
    #[snafu(display("label selector with binary operator {operator:?} must have values"))]
    LabelSelectorBinaryOperatorWithoutValues { operator: String },

    #[snafu(display("label selector with unary operator {operator:?} must not have values"))]
    LabelSelectorUnaryOperatorWithValues { operator: String },

    #[snafu(display("labelSelector has an invalid operator {operator:?}"))]
    LabelSelectorInvalidOperator { operator: String },
}

/// This trait extends the functionality of [`LabelSelector`].
///
/// Implementing this trait for any other type other than [`LabelSelector`]
/// can result in unndefined behaviour.
pub trait LabelSelectorExt {
    /// Takes a [`LabelSelector`] and converts it to a String that can be used
    /// in Kubernetes API calls. It will return an error if the LabelSelector
    /// contains illegal things (e.g. an `Exists` operator with a value).
    fn to_query_string(&self) -> Result<String>;
}

impl LabelSelectorExt for LabelSelector {
    fn to_query_string(&self) -> Result<String> {
        let mut query_string = String::new();

        // match_labels are the "old" part of LabelSelectors.
        // They are the equivalent for the "In" operator in match_expressions
        // In a query string each key-value pair will be separated by an "=" and the pairs
        // are then joined on commas.
        // The whole match_labels part is optional so we only do this if there are match labels.
        if let Some(label_map) = &self.match_labels {
            query_string.push_str(
                &label_map
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }

        // Match expressions are more complex than match labels, both can appear in the same API call
        // They support these operators: "In", "NotIn", "Exists" and "DoesNotExist"
        let expressions = self.match_expressions.as_ref().map(|requirements| {
            // If we had match_labels AND we have match_expressions we need to separate those two
            // with a comma.
            if !requirements.is_empty() && !query_string.is_empty() {
                query_string.push(',');
            }

            // Here we map over all requirements (which might be empty) and for each of the requirements
            // we create a Result<String, Error> with the Ok variant being the converted match expression
            // We then collect those Results into a single Result with the Error being the _first_ error.
            // This, unfortunately means, that we'll throw away all but one error.
            // TODO: Return all errors in one go: https://github.com/stackabletech/operator-rs/issues/127
            let expression_string: Result<Vec<String>> = requirements
                .iter()
                .map(|requirement| match requirement.operator.as_str() {
                    // In and NotIn can be handled the same, they both map to a simple "key OPERATOR (values)" string
                    operator @ "In" | operator @ "NotIn" => match &requirement.values {
                        Some(values) if !values.is_empty() => Ok(format!(
                            "{} {} ({})",
                            requirement.key,
                            operator.to_ascii_lowercase(),
                            values.join(", ")
                        )),
                        _ => Err(SelectorError::LabelSelectorBinaryOperatorWithoutValues {
                            operator: operator.to_owned(),
                        }),
                    },
                    // "Exists" is just the key and nothing else, if values have been specified it's an error
                    operator @ "Exists" => match &requirement.values {
                        Some(values) if !values.is_empty() => {
                            Err(SelectorError::LabelSelectorUnaryOperatorWithValues {
                                operator: operator.to_owned(),
                            })
                        }
                        _ => Ok(requirement.key.to_string()),
                    },
                    // "DoesNotExist" is similar to "Exists" but it is preceded by an exclamation mark
                    operator @ "DoesNotExist" => match &requirement.values {
                        Some(values) if !values.is_empty() => {
                            Err(SelectorError::LabelSelectorUnaryOperatorWithValues {
                                operator: operator.to_owned(),
                            })
                        }
                        _ => Ok(format!("!{key}", key = requirement.key)),
                    },
                    operator => Err(SelectorError::LabelSelectorInvalidOperator {
                        operator: operator.to_owned(),
                    }),
                })
                .collect();

            expression_string
        });

        if let Some(expressions) = expressions.transpose()? {
            query_string.push_str(&expressions.join(","));
        };

        Ok(query_string)
    }
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
                values: Some(vec!["bar".to_string()]),
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "In".to_string(),
                values: Some(vec!["quick".to_string(), "bar".to_string()]),
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "NotIn".to_string(),
                values: Some(vec!["quick".to_string(), "bar".to_string()]),
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "Exists".to_string(),
                values: None,
            },
            LabelSelectorRequirement {
                key: "foo".to_string(),
                operator: "DoesNotExist".to_string(),
                values: None,
            },
        ];

        let ls = LabelSelector {
            match_expressions: Some(match_expressions),
            match_labels: Some(match_labels.clone()),
        };
        assert_eq!(
            ls.to_query_string().unwrap(),
            "foo=bar,hui=buh,foo in (bar),foo in (quick, bar),foo notin (quick, bar),foo,!foo",
        );

        let ls = LabelSelector {
            match_expressions: None,
            match_labels: Some(match_labels),
        };
        assert_eq!(ls.to_query_string().unwrap(), "foo=bar,hui=buh",);

        let ls = LabelSelector {
            match_expressions: None,
            match_labels: None,
        };
        assert_eq!(ls.to_query_string().unwrap(), "");
    }

    #[test]
    #[should_panic]
    fn test_invalid_label_in_selector() {
        let match_expressions = vec![LabelSelectorRequirement {
            key: "foo".to_string(),
            operator: "In".to_string(),
            values: None,
        }];

        let ls = LabelSelector {
            match_expressions: Some(match_expressions),
            match_labels: None,
        };

        ls.to_query_string().unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_operator_in_selector() {
        let match_expressions = vec![LabelSelectorRequirement {
            key: "foo".to_string(),
            operator: "IllegalOperator".to_string(),
            values: None,
        }];

        let ls = LabelSelector {
            match_expressions: Some(match_expressions),
            match_labels: None,
        };

        ls.to_query_string().unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_exists_operator_in_selector() {
        let match_expressions = vec![LabelSelectorRequirement {
            key: "foo".to_string(),
            operator: "Exists".to_string(),
            values: Some(vec!["foobar".to_string()]),
        }];

        let ls = LabelSelector {
            match_expressions: Some(match_expressions),
            match_labels: None,
        };

        ls.to_query_string().unwrap();
    }
}
