//! This module deals with the [`Condition`] object from Kubernetes.
use chrono::Utc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::core::Resource;
use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};
use serde_json::json;
use std::fmt;

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

/// Schema for `.status.conditions` fields that annotates it as a map-list.
///
/// Must be applied (with `#[schemars(schema_with = ...)]`) in order to avoid conflicts when managing more than one condition
pub fn conditions_schema(gen: &mut SchemaGenerator) -> Schema {
    let mut schema = Vec::<Condition>::json_schema(gen);
    if let Schema::Object(schema) = &mut schema {
        schema
            .extensions
            .insert("x-kubernetes-list-type".to_string(), json!("map"));
        schema
            .extensions
            .insert("x-kubernetes-list-map-keys".to_string(), json!(["type"]));
    }
    schema
}
