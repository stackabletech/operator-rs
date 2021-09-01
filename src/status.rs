//! This module provides structs and trades to generalize the custom resource status access.
use crate::cluster_state::ClusterState;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;

pub trait Conditions {
    fn conditions(&self) -> Option<&[Condition]>;
    fn conditions_mut(&mut self) -> &mut Vec<Condition>;
}

pub trait Stateful<T> {
    fn cluster_state(&self) -> Option<ClusterState<T>>;
    fn cluster_state_mut(&mut self) -> &mut ClusterState<T>;
}
