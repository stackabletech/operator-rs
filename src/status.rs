//! This module provides structs and trades to generalize the custom resource status access.
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;

pub trait Conditions {
    fn conditions(&self) -> Option<&[Condition]>;
    fn conditions_mut(&mut self) -> &mut Vec<Condition>;
}
