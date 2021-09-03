//! This module provides structs and methods to provide some stateful data in the custom resource
//! status.
//!
//! Node assignments are stored in the status to provide 'sticky' pods and ids for scheduling pods
//! to nodes.
//!
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait Scheduler<T: PodIdentityGenerator> {
    fn schedule(
        id_generator: &T,
        nodes: &Vec<NodeIdentity>,
        last_mapping: &HashMap<PodIdentity, NodeIdentity>,
    ) -> HashMap<PodIdentity, NodeIdentity>;
}

pub trait PodIdentityGenerator {
    fn generate(&self) -> Vec<PodIdentity>;
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSchedulerHistory {
    pub history: HashMap<PodIdentity, NodeIdentity>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodIdentity {
    pub app: String,
    pub instance: String,
    pub role: String,
    pub group: String,
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdentity {
    pub name: String,
}

impl Display for NodeIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub struct StickyScheduler {
    pub history: Option<SimpleSchedulerHistory>,
    pub strategy: ScheduleStrategy,
}

pub enum ScheduleStrategy {
    GroupAntiAffinity,
}

impl StickyScheduler {
    pub fn new(history: Option<SimpleSchedulerHistory>, strategy: ScheduleStrategy) -> Self {
        StickyScheduler { history, strategy }
    }
}

impl<T> Scheduler<T> for StickyScheduler
where
    T: PodIdentityGenerator,
{
    fn schedule(
        id_generator: &T,
        nodes: &Vec<NodeIdentity>,
        last_mapping: &HashMap<PodIdentity, NodeIdentity>,
    ) -> HashMap<PodIdentity, NodeIdentity> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[test]
    fn test_cluster_state() {}
}
