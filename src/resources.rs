//! This module defines structs that can be referenced from operator CRDs to provide a common way of
//! defining resource limits across the Stackable platform.
//!
//! The structs define shared limits for memory and cpu resources, with the option of specifying
//! adding operator specific memory settings as well via a generic part of the struct.
//! For this generic part of the struct an empty default has been defined, if no extra options are
//! necessary.
//!
//! In addition to the mentioned limits the Resources struct is also generic over the storage
//! configuration, which allows operators to offer custom configuration here as needed for the
//! specific product.
//! For persistent storage it is recommended to base the storage config on the [`PvcConfig`] struct.
//!
//! We expect to define common additional runtime limits in this module over time, at the moment
//! JVM heap settings are the only available shared implementation.
//!
//! The following example shows how these structs can be used in the operator CRDs, this example
//! allows defining Jvm heap settings as part of the memory limits as well as three different PVC
//! configurations which can be used by the operator to request storage while provisioning.
//!
//! # Example
//!
//! ```no_run
//! #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
//! #[kube(
//! group = "product.stackable.tech",
//! version = "v1alpha1",
//! kind = "ProductCluster",
//! shortname = "product",
//! namespaced,
//! crates(
//! kube_core = "stackable_operator::kube::core",
//! k8s_openapi = "stackable_operator::k8s_openapi",
//! schemars = "stackable_operator::schemars"
//! )
//! )]
//! #[kube()]
//! #[serde(rename_all = "camelCase")]
//! pub struct ProductSpec {
//!     #[serde(default, skip_serializing_if = "Option::is_none")]
//!     pub nodes: Option<Role<ProductConfig>>,
//! }
//!
//! #[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
//! #[serde(rename_all = "camelCase")]
//! pub struct ProductConfig {
//!     resources: Option<Resources<ProductStorageConfig, JvmHeapLimits>>,
//! }
//!
//! pub struct ProductStorageConfig {
//!     data_storage: PvcConfig,
//!     metadata_storage: PvcConfig,
//!     shared_storage: PvcConfig,
//! }

use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// This struct allows specifying memory and cpu limits as well as generically adding storage
// settings.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resources<T, K = NoRuntimeLimits> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memory: Option<MemoryLimits<K>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cpu: Option<CpuLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    storage: Option<T>,
}

// Defines memory limits to be set on the pods
// Is generic to enable adding custom configuration for specific runtimes or products
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLimits<T> {
    // The maximum amount of memory that should be available
    // Should in most cases be mapped to resources.limits.memory
    limit: String,
    // Additional options that may be required
    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtime_limits: Option<T>,
}

// Default struct to allow operators not specifying `runtime_limits` when using [`MemoryLimits`]
pub struct NoRuntimeLimits {}

// Definition of Java Heap settings
// `min` is optional and should usually be defaulted to the same value as `max` by the implementing
// code
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JvmHeapLimits {
    max: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<String>,
}

// Cpu limits
// These should usually be forwarded to resources.limits.cpu
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuLimits {
    min: String,
    max: String,
}

// Struct that exposes the values for a PVC which the user should be able to influence
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PvcConfig {
    capacity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    storage_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selectors: Option<LabelSelector>,
}
