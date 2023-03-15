use std::collections::HashMap;

use k8s_openapi::{
    api::apps::v1::{DaemonSet, Deployment, StatefulSet},
    apimachinery::pkg::apis::meta::v1::Time,
    Resource,
};

use crate::cluster_resources::ClusterResource;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait HasStatus: ClusterResource {}

impl HasStatus for StatefulSet {}
impl HasStatus for Deployment {}
impl HasStatus for DaemonSet {}

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
#[derive(Default, PartialEq)]
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
pub trait StatusBuilder<T: Resource> {
    fn conditions(resource: &T, crs: &ClusterResourcesStatus) -> Vec<ClusterCondition>;
}

struct ADR27StatusBuilder {}

impl<T: Resource> StatusBuilder<T> for ADR27StatusBuilder {
    fn conditions(_resource: &T, crs: &ClusterResourcesStatus) -> Vec<ClusterCondition> {
        let mut result: HashMap<ClusterConditionType, ClusterConditionStatus> = HashMap::new();

        let (sts_replicas, sts_available_replicas) = crs
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

        result.insert(
            ClusterConditionType::Available,
            (sts_replicas + deploy_replicas == sts_available_replicas + deploy_available_replicas)
                .into(),
        );

        result
            .into_iter()
            .map(|(type_, status)| ClusterCondition::new_with(type_, status))
            .collect()
    }
}
