use std::collections::HashMap;

use k8s_openapi::{
    api::{
        apps::v1::{
            DaemonSet, DaemonSetStatus, Deployment, DeploymentStatus, StatefulSet,
            StatefulSetStatus,
        },
        core::v1::PodStatus,
    },
    apimachinery::pkg::apis::meta::v1::Time,
};

use kube::Resource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    strum::Display, Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ClusterConditionType {
    #[default]
    Available,
    Degraded,
    Progressing,
    Paused,
    Stopped,
}

#[derive(
    strum::Display, Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ClusterConditionStatus {
    #[default]
    True,
    False,
    Unknown,
}

impl From<bool> for ClusterConditionStatus {
    fn from(b: bool) -> ClusterConditionStatus {
        if b {
            ClusterConditionStatus::True
        } else {
            ClusterConditionStatus::False
        }
    }
}
#[derive(Debug, Default, PartialEq)]
pub struct ClusterResourcesStatus {
    pub stateful_set_status: Vec<StatefulSetStatus>,
    pub daemon_set_status: Vec<DaemonSetStatus>,
    pub deployment_status: Vec<DeploymentStatus>,
    pub pod_status: Vec<PodStatus>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClusterCondition {
    /// Last time the condition transitioned from one status to another.
    pub last_transition_time: Option<Time>,

    /// The last time this condition was updated.
    pub last_update_time: Option<Time>,

    /// A human readable message indicating details about the transition.
    pub message: Option<String>,

    /// The reason for the condition's last transition.
    pub reason: Option<String>,

    /// Status of the condition, one of True, False, Unknown.
    pub status: ClusterConditionStatus,

    /// Type of deployment condition.
    pub type_: ClusterConditionType,
}

impl ClusterCondition {
    pub fn new_with(type_: ClusterConditionType, status: ClusterConditionStatus) -> Self {
        ClusterCondition {
            type_,
            status,
            ..ClusterCondition::default()
        }
    }
}

pub trait HasStop {
    fn is_stopped(&self) -> bool {
        false
    }
}
pub trait HasPause {
    fn is_paused(&self) -> bool {
        false
    }
}

trait HasStatus {
    fn available(&self) -> Option<ClusterCondition> {
        None
    }
}

impl HasStatus for StatefulSet {
    fn available(&self) -> Option<ClusterCondition> {
        let requested_replicas = self.spec.and_then(|spec| spec.replicas).unwrap_or_default();
        let available_replicas = self.status.and_then(|status| status.available_replicas).unwrap_or_default();
        Some(ClusterCondition {
            last_transition_time: None,
            last_update_time: None,
            message: Some("".to_owned()),
            reason: Some("".to_owned()),
            status: (requested_replicas == available_replicas).into(),
            type_: ClusterConditionType::Available })
    }

}

impl HasStatus for DaemonSet {
    fn available(&self) -> Option<ClusterCondition> {
        let requested_replicas = self.status.and_then(|spec| Some(spec.desired_number_scheduled)).unwrap_or_default();
        let available_replicas = self.status.and_then(|status| Some(status.number_ready)).unwrap_or_default();
        Some(ClusterCondition {
            last_transition_time: None,
            last_update_time: None,
            message: Some("".to_owned()),
            reason: Some("".to_owned()),
            status: (requested_replicas == available_replicas).into(),
            type_: ClusterConditionType::Available })
    }

}

impl HasStatus for Deployment {
    fn available(&self) -> Option<ClusterCondition> {
        let requested_replicas = self.spec.and_then(|spec| spec.replicas).unwrap_or_default();
        let available_replicas = self.status.and_then(|status| status.ready_replicas).unwrap_or_default();
        Some(ClusterCondition {
            last_transition_time: None,
            last_update_time: None,
            message: Some("".to_owned()),
            reason: Some("".to_owned()),
            status: (requested_replicas == available_replicas).into(),
            type_: ClusterConditionType::Available })
    }

}

pub trait ClusterStatus: HasStatus {
    fn condtions(&self) -> Vec<ClusterCondition>;
}

fn compute_cluster_conditions(resources: &[dyn HasStatus]) -> Vec<ClusterCondition> {
    todo!()
}


pub trait ClusterStatusBuilder {
    type Status;

    fn available<T: HasStatus>(
        &self,
        resource: &[T],
    ) -> Option<ClusterCondition> {
/*
        let (requested_replicas, sts_available_replicas) = crs
            .stateful_set_status
            .iter()
            .map(|s| (s.replicas, s.available_replicas.unwrap_or_default()))
            .reduce(|(mut tr, mut tar), (r, ar)| {
                tr += r;
                tar += ar;
                (tr, tar)
            })
            .unwrap_or((0, 0));

        let (deploy_replicas, deploy_available_replicas) = crs
            .deployment_status
            .iter()
            .map(|s| {
                (
                    s.replicas.unwrap_or_default(),
                    s.available_replicas.unwrap_or_default(),
                )
            })
            .reduce(|(mut tr, mut tar), (r, ar)| {
                tr += r;
                tar += ar;
                (tr, tar)
            })
            .unwrap_or((0, 0));

        Some(ClusterCondition {
            type_: ClusterConditionType::Available,
            status: (sts_replicas + deploy_replicas
                == sts_available_replicas + deploy_available_replicas)
                .into(),
        })
        */ None
    }

    fn progressing<T: Resource>(
        &self,
        resource: &T,
        crs: &ClusterResourcesStatus,
    ) -> Option<ClusterCondition> {
        None
    }

    fn degraded<T: Resource>(
        &self,
        resource: &T,
        crs: &ClusterResourcesStatus,
    ) -> Option<ClusterCondition> {
        None
    }

    fn paused<T: Resource + HasPause>(
        &self,
        resource: &T,
        crs: &ClusterResourcesStatus,
    ) -> Option<ClusterCondition> {
        None
    }

    fn stopped<T: Resource + HasStop>(
        &self,
        resource: &T,
        crs: &ClusterResourcesStatus,
    ) -> Option<ClusterCondition> {
        None
    }

    fn conditions<T: Resource + HasPause>(
        &self,
        resource: &T,
        crs: &ClusterResourcesStatus,
    ) -> Vec<ClusterCondition> {
        let mut result = vec![];

        if let Some(condition) = self.available(resource, crs) {
            if condition.status != old_condition.status {
                // set transition time

            }
            result.push(condition);
        }

        result
    }

    fn status(&self, resource: &T) -> Status {
        let c = self.conditions()
    }
}
