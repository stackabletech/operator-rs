pub mod condition;

use chrono::Utc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use strum::EnumCount;

/// A **data structure** that contains a vector of `ClusterCondition`s.
/// Should usually be the status of a `CustomResource`.
pub trait HasStatusCondition {
    fn conditions(&self) -> Vec<ClusterCondition>;
}

pub trait ConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet;
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Last time the condition transitioned from one status to another.
    pub last_transition_time: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The last time this condition was updated.
    pub last_update_time: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A human readable message indicating details about the transition.
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The reason for the condition's last transition.
    pub reason: Option<String>,
    /// Status of the condition, one of True, False, Unknown.
    pub status: ClusterConditionStatus,
    /// Type of deployment condition.
    #[serde(rename = "type")]
    pub type_: ClusterConditionType,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    EnumCount,
    Eq,
    Hash,
    JsonSchema,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum ClusterConditionType {
    #[default]
    /// Available indicates that the binary maintained by the operator (eg: zookeeper for the
    /// zookeeper-operator), is functional and available in the cluster.
    Available,
    /// Degraded indicates that the operand is not functioning completely. An example of a degraded
    /// state would be if there should be 5 copies of the operand running but only 4 are running.
    /// It may still be available, but it is degraded.
    Degraded,
    /// Progressing indicates that the operator is actively making changes to the binary maintained
    /// by the operator (eg: zookeeper for the zookeeper-operator).
    Progressing,
    /// Paused indicates that the operator is not reconciling the cluster. This may be used for
    /// debugging or operator updating.
    Paused,
    /// Stopped indicates that all the cluster replicas are scaled down to 0. All resources (e.g.
    /// ConfigMaps, Services etc.) are kept.
    Stopped,
}

#[derive(
    Clone, Debug, Default, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum ClusterConditionStatus {
    #[default]
    /// True means a resource is in the condition.
    True,
    /// False means a resource is not in the condition.
    False,
    /// Unknown means kubernetes cannot decide if a resource is in the condition or not.
    Unknown,
}

#[derive(Default)]
/// Helper to order and merge `ClusterCondition` objects.
pub struct ClusterConditionSet {
    conditions: Vec<Option<ClusterCondition>>,
}

impl ClusterConditionSet {
    pub fn new() -> Self {
        ClusterConditionSet {
            // We use this as a quasi "Set" where each ClusterConditionType has its fixed position
            // This ensures ordering, and in contrast to e.g. a
            // BTreeMap<ClusterConditionType, ClusterCondition>, prevents shenanigans like adding a
            // ClusterCondition (as value) with a different ClusterConditionType than its key.
            // See "put".
            conditions: vec![None; ClusterConditionType::COUNT],
        }
    }

    /// Adds a ClusterCondition to its assigned index in the conditions vector.
    fn put(&mut self, condition: ClusterCondition) {
        let index = condition.type_ as usize;
        self.conditions[index] = Some(condition);
    }

    pub fn merge(self, other: ClusterConditionSet) -> ClusterConditionSet {
        let mut result = ClusterConditionSet::new();

        for (old_condition, new_condition) in self
            .conditions
            .into_iter()
            .zip(other.conditions.into_iter())
        {
            if let Some(condition) = match (old_condition, new_condition) {
                (Some(old), Some(new)) => Some(merge_condition(old, new)),
                (Some(old), None) => Some(old),
                (None, Some(new)) => Some(new),
                _ => None,
            } {
                result.put(condition);
            };
        }

        result
    }
}

impl From<ClusterConditionSet> for Vec<ClusterCondition> {
    fn from(value: ClusterConditionSet) -> Self {
        value.conditions.into_iter().flatten().collect()
    }
}

impl From<Vec<ClusterCondition>> for ClusterConditionSet {
    fn from(value: Vec<ClusterCondition>) -> Self {
        let mut result = ClusterConditionSet::new();
        for c in value {
            result.put(c);
        }
        result
    }
}

pub fn compute_conditions<T: ConditionBuilder, R: HasStatusCondition>(
    resource: &R,
    condition_builder: &[T],
) -> Vec<ClusterCondition> {
    let mut old_conditions: ClusterConditionSet = resource.conditions().into();

    for cb in condition_builder {
        let new_conditions: ClusterConditionSet = cb.build_conditions();

        old_conditions = old_conditions.merge(new_conditions);
    }

    old_conditions.into()
}

fn merge_condition(
    old_condition: ClusterCondition,
    new_condition: ClusterCondition,
) -> ClusterCondition {
    let now = Time(Utc::now());
    // No change in status -> update "last_update_time" and keep "last_transition_time"
    if old_condition.status == new_condition.status {
        ClusterCondition {
            last_update_time: Some(now),
            last_transition_time: old_condition.last_transition_time,
            ..new_condition
        }
    // Change in status -> set new "last_update_time" and "last_transition_time"
    } else {
        ClusterCondition {
            last_update_time: Some(now.clone()),
            last_transition_time: Some(now),
            ..new_condition
        }
    }
}
#[cfg(test)]
mod test {
    use crate::status::*;

    struct TestResource {}
    impl HasStatusCondition for TestResource {
        fn conditions(&self) -> Vec<ClusterCondition> {
            vec![ClusterCondition {
                type_: crate::status::ClusterConditionType::Available,
                status: crate::status::ClusterConditionStatus::False,
                message: Some("OMG! Thing is broken!".into()),
                ..ClusterCondition::default()
            }]
        }
    }
    struct TestConditionBuilder {}
    impl ConditionBuilder for TestConditionBuilder {
        fn build_conditions(&self) -> ClusterConditionSet {
            vec![ClusterCondition {
                type_: crate::status::ClusterConditionType::Available,
                status: crate::status::ClusterConditionStatus::True,
                message: Some("Relax. Everything is fine.".into()),
                ..ClusterCondition::default()
            }]
            .into()
        }
    }
    #[test]
    pub fn test_compute_conditions_with_transition() {
        let resource = TestResource {};
        let condition_builders = vec![TestConditionBuilder {}];
        let got = compute_conditions(&resource, &condition_builders)
            .get(0)
            .cloned()
            .unwrap();
        let expected = ClusterCondition {
            type_: crate::status::ClusterConditionType::Available,
            status: crate::status::ClusterConditionStatus::True,
            message: Some("Relax. Everything is fine.".into()),
            ..ClusterCondition::default()
        };

        assert_eq!(got.type_, expected.type_);
        assert_eq!(got.status, expected.status);
        assert_eq!(got.message, expected.message);
        assert_eq!(got.last_transition_time, got.last_update_time);
        assert!(got.last_transition_time.is_some());
    }
}
