//! This module provides structs and trades to generalize the custom resource status access.
use crate::versioning::Version;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;

pub trait Conditions {
    fn conditions(&self) -> &[Condition];
    fn conditions_mut(&mut self) -> &mut Vec<Condition>;
}

pub trait Versioned<V> {
    fn version(&self) -> &Option<Version<V>>;
    fn version_mut(&mut self) -> &mut Option<Version<V>>;
}
