pub mod condition;

use chrono::Utc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub trait HasStatusCondition {
    fn conditions(&self) -> Vec<ClusterCondition>;
}

#[derive(Default)]
pub struct ClusterConditionSet {
    conditions: Vec<Option<ClusterCondition>>,
}

impl ClusterConditionSet {
    pub fn new() -> Self {
        ClusterConditionSet {
            conditions: vec![None, None, None, None, None],
        }
    }
    fn put(&mut self, condition: ClusterCondition) {
        self.conditions[condition.type_ as usize] = Some(condition.clone());
    }

    pub fn merge(&mut self, other: &ClusterConditionSet) -> ClusterConditionSet {
        let mut result = ClusterConditionSet::new();
        for (old_condition, new_condition) in self.conditions.iter().zip(other.conditions.iter()) {
            let c = match (old_condition, new_condition) {
                (Some(old), Some(new)) => Some(merge_condition(old, new)),
                (Some(old), None) => Some(old.clone()),
                (None, Some(new)) => Some(new.clone()),
                _ => None,
            };
            if c.is_some() {
                result.put(c.unwrap());
            }
        }
        result
    }

    pub fn to_vec(&self) -> Vec<ClusterCondition> {
        self.conditions.clone().into_iter().flatten().collect()
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
pub trait ConditionBuilder {
    fn build_conditions(&self) -> ClusterConditionSet;
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, JsonSchema, Deserialize, Serialize)]
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

pub fn compute_conditions<T: ConditionBuilder, R: HasStatusCondition>(
    resource: &R,
    condition_builder: &[T],
) -> Vec<ClusterCondition> {
    let mut old_conditions: ClusterConditionSet = resource.conditions().into();

    for cb in condition_builder {
        let new_conditions: ClusterConditionSet = cb.build_conditions();

        old_conditions = old_conditions.merge(&new_conditions);
    }

    old_conditions.to_vec()
}

fn merge_condition(
    old_condition: &ClusterCondition,
    new_condition: &ClusterCondition,
) -> ClusterCondition {
    let now = Time(Utc::now());
    // No change in status -> update "last_update_time"
    if old_condition.status == new_condition.status {
        ClusterCondition {
            last_update_time: Some(now),
            last_transition_time: old_condition.last_transition_time.clone(),
            ..new_condition.clone()
        }
        // Change in status -> set new message, status and update / transition times
    } else {
        ClusterCondition {
            last_update_time: Some(now.clone()),
            last_transition_time: Some(now),
            ..new_condition.clone()
        }
    }
    // No condition available -> create
}
