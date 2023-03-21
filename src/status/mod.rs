use chrono::Utc;
use k8s_openapi::{
    api::apps::v1::{DaemonSet, StatefulSet},
    apimachinery::pkg::apis::meta::v1::Time,
};
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::{BTreeMap, HashMap};
use tracing::info;

#[derive(
    Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
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

pub trait HasCondition {
    fn conditions(&self) -> Vec<ClusterCondition>;
}

pub trait ConditionBuilder {
    fn build_conditions(&self) -> Vec<ClusterCondition>;
}

pub struct StatefulSetConditionBuilder<'a, T: HasCondition> {
    resource: &'a T,
    stateful_sets: Vec<StatefulSet>,
}

impl<'a, T: HasCondition> StatefulSetConditionBuilder<'a, T> {
    pub fn new(resource: &'a T) -> StatefulSetConditionBuilder<T> {
        StatefulSetConditionBuilder {
            resource,
            stateful_sets: Vec::new(),
        }
    }

    pub fn add(&mut self, sts: StatefulSet) {
        self.stateful_sets.push(sts);
    }

    pub fn available(&self) -> ClusterCondition {
        let opt_old_available = self
            .resource
            .conditions()
            .iter()
            .find(|cond| cond.type_ == ClusterConditionType::Available)
            .cloned();

        let mut sts_available = ClusterConditionStatus::True;
        for sts in &self.stateful_sets {
            sts_available = cmp::max(sts_available, stateful_set_available(sts));
        }

        let message = match sts_available {
            ClusterConditionStatus::True => "cluster has the requested amount of ready replicas",
            ClusterConditionStatus::False => {
                "cluster does not have the requested amount of ready replicas"
            }
            ClusterConditionStatus::Unknown => "Unknown",
        };

        update_condition(
            ClusterConditionType::Available,
            opt_old_available,
            sts_available,
            message,
        )
    }
}

fn update_condition(
    condition_type: ClusterConditionType,
    old_condition: Option<ClusterCondition>,
    merged_condition_status: ClusterConditionStatus,
    message: &str,
) -> ClusterCondition {
    let now = Time(Utc::now());
    if let Some(old_condition) = old_condition {
        // No change in status -> update "last_update_time"
        if old_condition.status == merged_condition_status {
            ClusterCondition {
                last_update_time: Some(now),
                ..old_condition
            }
        // Change in status -> set new message, status and update / transition times
        } else {
            ClusterCondition {
                last_update_time: Some(now.clone()),
                last_transition_time: Some(now),
                status: merged_condition_status,
                message: Some(message.to_string()),
                ..old_condition
            }
        }
    // No condition available -> create
    } else {
        ClusterCondition {
            last_update_time: Some(now.clone()),
            last_transition_time: Some(now),
            status: merged_condition_status,
            message: Some(message.to_string()),
            reason: None,
            type_: condition_type,
        }
    }
}

impl<'a, T: HasCondition> ConditionBuilder for StatefulSetConditionBuilder<'a, T> {
    fn build_conditions(&self) -> Vec<ClusterCondition> {
        vec![self.available()]
    }
}

pub struct DaemonSetConditionBuilder<'a, T: HasCondition> {
    resource: &'a T,
    daemon_sets: Vec<DaemonSet>,
}

impl<'a, T: HasCondition> DaemonSetConditionBuilder<'a, T> {
    pub fn new(resource: &'a T) -> DaemonSetConditionBuilder<T> {
        DaemonSetConditionBuilder {
            resource,
            daemon_sets: Vec::new(),
        }
    }

    pub fn add(&mut self, ds: DaemonSet) {
        self.daemon_sets.push(ds);
    }

    pub fn available(&self) -> ClusterCondition {
        let opt_old_available = self
            .resource
            .conditions()
            .iter()
            .find(|cond| cond.type_ == ClusterConditionType::Available)
            .cloned();

        let mut ds_available = ClusterConditionStatus::True;
        for ds in &self.daemon_sets {
            ds_available = cmp::max(ds_available, daemon_set_available(ds));
        }

        let message = match ds_available {
            ClusterConditionStatus::True => "cluster has the requested amount of ready replicas",
            ClusterConditionStatus::False => {
                "cluster does not have the requested amount of ready replicas"
            }
            ClusterConditionStatus::Unknown => "Unknown",
        };

        update_condition(
            ClusterConditionType::Available,
            opt_old_available,
            ds_available,
            message,
        )
    }
}

impl<'a, T: HasCondition> ConditionBuilder for DaemonSetConditionBuilder<'a, T> {
    fn build_conditions(&self) -> Vec<ClusterCondition> {
        vec![self.available()]
    }
}

pub fn compute_conditions<T: ConditionBuilder>(condition_builder: &[T]) -> Vec<ClusterCondition> {
    let mut current_conditions = BTreeMap::<ClusterConditionType, ClusterCondition>::new();
    for cb in condition_builder {
        let cb_conditions: HashMap<ClusterConditionType, ClusterCondition> = cb
            .build_conditions()
            .iter()
            .map(|c| (c.type_.clone(), c.clone()))
            .collect();

        for (current_condition_type, cb_condition) in cb_conditions {
            let current_condition = current_conditions.get(&current_condition_type);

            let next_condition = if let Some(current) = current_condition {
                if current.status > cb_condition.status {
                    current
                } else {
                    &cb_condition
                }
            } else {
                &cb_condition
            };

            current_conditions.insert(current_condition_type, next_condition.clone());
        }
    }

    current_conditions.values().cloned().collect()
}

fn stateful_set_available(sts: &StatefulSet) -> ClusterConditionStatus {
    let requested_replicas = sts
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or_default();
    let available_replicas = sts
        .status
        .as_ref()
        .and_then(|status| status.available_replicas)
        .unwrap_or_default();

    info!("STS: requested_replicas={requested_replicas}");
    info!("STS: available_replicas={available_replicas}");

    if requested_replicas == available_replicas {
        ClusterConditionStatus::True
    } else {
        ClusterConditionStatus::False
    }
}

fn daemon_set_available(ds: &DaemonSet) -> ClusterConditionStatus {
    let desired_number_scheduled = ds
        .status
        .as_ref()
        .map(|status| status.desired_number_scheduled)
        .unwrap_or_default();
    let number_ready = ds
        .status
        .as_ref()
        .map(|status| status.number_ready)
        .unwrap_or_default();

    info!("Ds: desired_number_scheduled={desired_number_scheduled}");
    info!("Ds: number_ready={number_ready}");
    if desired_number_scheduled == number_ready {
        ClusterConditionStatus::True
    } else {
        ClusterConditionStatus::False
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {}
}
