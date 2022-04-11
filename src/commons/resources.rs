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
//! ```ignore
//! use stackable_operator::role_utils::Role;
//! use stackable_operator::resources::{Resources, PvcConfig, JvmHeapLimits};
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//! use kube::CustomResource;
//!
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

use k8s_openapi::api::core::v1::{
    PersistentVolumeClaim, PersistentVolumeClaimSpec, ResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// This struct allows specifying memory and cpu limits as well as generically adding storage
// settings.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resources<T, K = NoRuntimeLimits>
where
    T: Clone,
    K: Clone,
{
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryLimits<K>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<T>,
}

// Defines memory limits to be set on the pods
// Is generic to enable adding custom configuration for specific runtimes or products
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLimits<T>
where
    T: Clone,
{
    // The maximum amount of memory that should be available
    // Should in most cases be mapped to resources.limits.memory
    pub limit: String,
    // Additional options that may be required
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_limits: Option<T>,
}

// Default struct to allow operators not specifying `runtime_limits` when using [`MemoryLimits`]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NoRuntimeLimits {}

// Definition of Java Heap settings
// `min` is optional and should usually be defaulted to the same value as `max` by the implementing
// code
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JvmHeapLimits {
    pub max: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
}

// Cpu limits
// These should usually be forwarded to resources.limits.cpu
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuLimits {
    pub min: String,
    pub max: String,
}

// Struct that exposes the values for a PVC which the user should be able to influence
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PvcConfig {
    pub capacity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selectors: Option<LabelSelector>,
}

impl PvcConfig {
    /// Create a PVC from this PvcConfig
    pub fn build_pvc(&self, name: &str, access_modes: Option<Vec<&str>>) -> PersistentVolumeClaim {
        PersistentVolumeClaim {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..ObjectMeta::default()
            },
            spec: Some(PersistentVolumeClaimSpec {
                access_modes: access_modes
                    .map(|modes| modes.into_iter().map(String::from).collect()),
                resources: Some(ResourceRequirements {
                    requests: Some({
                        let mut map = BTreeMap::new();
                        map.insert("storage".to_string(), Quantity(self.capacity.to_string()));
                        map
                    }),
                    ..ResourceRequirements::default()
                }),
                ..PersistentVolumeClaimSpec::default()
            }),
            ..PersistentVolumeClaim::default()
        }
    }
}

// Since we don't own ResourceRequirement we implement Into instead of From
#[allow(clippy::from_over_into)]
impl<T, K> Into<ResourceRequirements> for Resources<T, K>
where
    T: Clone,
    K: Clone,
{
    fn into(self) -> ResourceRequirements {
        let mut limits = BTreeMap::new();
        let mut requests = BTreeMap::new();
        if let Some(memory_limit) = self.memory {
            limits.insert(
                "memory".to_string(),
                Quantity(memory_limit.limit.to_string()),
            );
            requests.insert("memory".to_string(), Quantity(memory_limit.limit));
        }

        if let Some(cpu_limit) = self.cpu {
            limits.insert("cpu".to_string(), Quantity(cpu_limit.max));
            requests.insert("cpu".to_string(), Quantity(cpu_limit.min));
        }

        ResourceRequirements {
            limits: Some(limits),
            requests: Some(requests),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commons::resources::{PvcConfig, Resources};
    use k8s_openapi::api::core::v1::{PersistentVolumeClaim, ResourceRequirements};
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct TestStorageConfig {}

    #[rstest]
    #[case::no_access(
        "test",
        None,
        r#"
        capacity: 10Gi"#,
        r#"
        apiVersion: v1
        kind: PersistentVolumeClaim
        metadata:
            name: test
        spec:
            resources:
                requests:
                    storage: 10Gi"#
    )]
    #[case::access_readmany(
        "test2",
        Some(vec!["ReadWriteMany"]),
        r#"
        capacity: 100Gi"#,
        r#"
        apiVersion: v1
        kind: PersistentVolumeClaim
        metadata:
            name: test2
        spec:
            accessModes:
                - ReadWriteMany
            resources:
                requests:
                    storage: 100Gi"#
    )]
    #[case::multiple_accessmodes(
        "testtest",
        Some(vec!["ReadWriteMany", "ReadOnlyMany"]),
        r#"
        capacity: 200Gi"#,
        r#"
        apiVersion: v1
        kind: PersistentVolumeClaim
        metadata:
            name: testtest
        spec:
            accessModes:
                - ReadWriteMany
                - ReadOnlyMany
            resources:
                requests:
                    storage: 200Gi"#
    )]
    #[case::storage_class(
        "test",
        None,
        r#"
        capacity: 10Gi
        storageClass: CustomClass"#,
        r#"
        apiVersion: v1
        kind: PersistentVolumeClaim
        metadata:
            name: test
        spec:
            storageClass: CustomClass
            resources:
                requests:
                    storage: 10Gi"#
    )]

    fn test_build_pvc(
        #[case] name: String,
        #[case] access_modes: Option<Vec<&str>>,
        #[case] input: String,
        #[case] expected: String,
    ) {
        let input_pvcconfig: PvcConfig = serde_yaml::from_str(&input).expect("illegal test input");

        let result = input_pvcconfig.build_pvc(&name, access_modes);

        let expected_volumeclaim: PersistentVolumeClaim =
            serde_yaml::from_str(&expected).expect("illegal expected output");

        assert_eq!(result, expected_volumeclaim);
    }

    #[rstest]
    #[case::only_memlimits(
        r#"
        memory:
            limit: 1Gi"#,
        r#"
        limits:
            memory: 1Gi
        requests:
            memory: 1Gi"#
    )]
    #[case::only_cpulimits(
        r#"
        cpu:
            min: 1000
            max: 2000"#,
        r#"
        limits:
            cpu: 2000
        requests:
            cpu: 1000"#
    )]
    #[case::mem_and_cpu_limits(
        r#"
        cpu:
            min: 1000
            max: 2000
        memory:
            limit: 20Gi"#,
        r#"
        limits:
            memory: 20Gi
            cpu: 2000
        requests:
            memory: 20Gi
            cpu: 1000"#
    )]
    fn test_into_resourcelimits(#[case] input: String, #[case] expected: String) {
        let input_resources: Resources<TestStorageConfig> =
            serde_yaml::from_str(&input).expect("illegal test input");

        let result: ResourceRequirements = input_resources.into();
        let expected_requirements: ResourceRequirements =
            serde_yaml::from_str(&expected).expect("illegal expected output");

        assert_eq!(result, expected_requirements);
    }
}
