use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

use crate::cluster_resources::ClusterResourcesStatus;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    strum::Display, Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize,
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

pub trait StatusBuilder {
    fn conditions(crs: &ClusterResourcesStatus) -> Vec<ClusterCondition>;
}

struct ADR27StatusBuilder {}

impl StatusBuilder for ADR27StatusBuilder {
    fn conditions(crs: &ClusterResourcesStatus) -> Vec<ClusterCondition> {
        vec![]
    }
}
