//! This module deals with the [`Condition`] object from Kubernetes.
use chrono::Utc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::Resource;
use schemars::gen::SchemaGenerator;
use schemars::schema::Schema;
use serde_json::{from_value, json};
use std::fmt;

/// Returns a [`Schema`] that can be used with custom Conditions which have the same structure
/// as the `io.k8s.pkg.apis.meta.v1.Condition` resource from Kubernetes.
///
/// This is needed because the [`Condition`] from `k8s-openapi` does not derive `JsonSchema`.
///
/// # Example
///
/// ```
/// use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
/// use schemars::JsonSchema;
///
/// #[derive(JsonSchema)]
/// #[serde(rename_all = "camelCase")]
/// pub struct FooCrd {
///     #[serde(default, skip_serializing_if = "Vec::is_empty")]
///     #[schemars(schema_with = "stackable_operator::conditions::schema")]
///     pub conditions: Vec<Condition>,
/// }
/// ```
pub fn schema(_: &mut SchemaGenerator) -> Schema {
    from_value(json!({
        "type": "array",
        "x-kubernetes-list-type": "map",
        "x-kubernetes-list-map-keys": ["type"],
        "x-kubernetes-patch-strategy": "merge",
        "x-kubernetes-patch-merge-key": "type",
        "items": {
            "type": "object",
            "properties": {
                "lastTransitionTime": {
                    "description": "lastTransitionTime is the last time the condition transitioned from one status to another. This should be when the underlying condition changed.  If that is not known, then using the time when the API field changed is acceptable.",
                    "format": "date-time",
                    "type": "string"
                },
                "message": {
                    "description": "message is a human readable message indicating details about the transition. This may be an empty string.",
                    "type": "string"
                },
                "observedGeneration": {
                    "description": "observedGeneration represents the .metadata.generation that the condition was set based upon. For instance, if .metadata.generation is currently 12, but the .status.conditions[x].observedGeneration is 9, the condition is out of date with respect to the current state of the instance.",
                    "format": "int64",
                    "type": "integer"
                },
                "reason": {
                    "description": "reason contains a programmatic identifier indicating the reason for the condition's last transition. Producers of specific condition types may define expected values and meanings for this field, and whether the values are considered a guaranteed API. The value should be a CamelCase string. This field may not be empty.",
                    "type": "string"
                },
                "status": {
                    "default": "Unknown",
                    "description": "status of the condition, one of True, False, Unknown.",
                    "enum": [
                        "Unknown",
                        "True",
                        "False"
                    ],
                    "type": "string"
                },
                "type": {
                    "description": "type of condition in CamelCase or in foo.example.com/CamelCase.",
                    "pattern": "^([A-Za-z0-9][-A-Za-z0-9_.]*)?[A-Za-z0-9]$",
                    "type": "string"
                }
            },
            "required": [
                "type",
                "status",
                "lastTransitionTime",
                "reason",
                "message"
            ],
        },
    }))
    .unwrap()
}

/// According to the Kubernetes schema the only allowed values for the `status` of a `Condition`
/// are `True`, `False` and `Unknown`.
pub enum ConditionStatus {
    True,
    False,
    Unknown,
}

impl fmt::Display for ConditionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConditionStatus::True => write!(f, "True"),
            ConditionStatus::False => write!(f, "False"),
            ConditionStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Builds a [`Condition`] from the passed in parameters.
///
/// * It uses the `current_conditions` to set the `last_transition_time` field automatically
/// * It uses the passed in `resource` to automatically set the `observed_generation`
/// * The remaining parameters are just passed through
pub fn build_condition<T>(
    resource: &T,
    current_conditions: Option<&[Condition]>,
    message: String,
    reason: String,
    status: ConditionStatus,
    condition_type: String,
) -> Condition
where
    T: Resource,
{
    // In these two let statements we check if the same condition was already set and if the
    // status is different or not.
    // Only if the status is different do we update the `last_transition_time`.
    let old_condition = current_conditions.and_then(|old_condition| {
        old_condition
            .iter()
            .find(|condition| condition.type_ == condition_type)
    });

    let last_transition_time = match old_condition {
        Some(condition) if condition.status == status.to_string() => {
            condition.last_transition_time.clone()
        }
        _ => Time(Utc::now()),
    };

    Condition {
        last_transition_time,
        message,
        observed_generation: resource.meta().generation,
        reason,
        status: status.to_string(),
        type_: condition_type,
    }
}
