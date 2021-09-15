//! This module provides structs and trades to generalize the custom resource status access.
use crate::versioning::ProductVersion;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;

/// Provides access to the custom resource status conditions.
pub trait Conditions {
    fn conditions(&self) -> &[Condition];
    fn conditions_mut(&mut self) -> &mut Vec<Condition>;
}

/// Provides access to the custom resource status version for up or downgrades.
pub trait Versioned<V> {
    fn version(&self) -> &Option<ProductVersion<V>>;
    fn version_mut(&mut self) -> &mut Option<ProductVersion<V>>;
}
