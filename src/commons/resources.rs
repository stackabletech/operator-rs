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
//! ```
//! use stackable_operator::config::fragment::Fragment;
//! use stackable_operator::role_utils::Role;
//! use stackable_operator::commons::resources::{Resources, PvcConfig, JvmHeapLimits};
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//! use kube::CustomResource;
//!
//! #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, Serialize)]
//! #[kube(
//!     group = "product.stackable.tech",
//!     version = "v1alpha1",
//!     kind = "ProductCluster",
//!     shortname = "product",
//!     namespaced,
//!     crates(
//!         kube_core = "stackable_operator::kube::core",
//!         k8s_openapi = "stackable_operator::k8s_openapi",
//!         schemars = "stackable_operator::schemars"
//!     )
//! )]
//! #[serde(rename_all = "camelCase")]
//! pub struct ProductSpec {
//!     #[serde(default, skip_serializing_if = "Option::is_none")]
//!     pub nodes: Option<Role<ProductConfigFragment>>,
//! }
//!
//! #[derive(Debug, Default, PartialEq, Fragment, JsonSchema)]
//! #[fragment_attrs(
//!     derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema),
//!     serde(rename_all = "camelCase"),
//! )]
//! pub struct ProductConfig {
//!     resources: Resources<ProductStorageConfig, JvmHeapLimits>,
//! }
//!
//! #[derive(Debug, Default, PartialEq, Fragment, JsonSchema)]
//! #[fragment_attrs(
//!     derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema),
//!     serde(rename_all = "camelCase"),
//! )]
//! pub struct ProductStorageConfig {
//!     data_storage: PvcConfig,
//!     metadata_storage: PvcConfig,
//!     shared_storage: PvcConfig,
//! }
//! ```

use crate::config::{
    fragment::{Fragment, FromFragment},
    merge::Merge,
};
use derivative::Derivative;
use k8s_openapi::api::core::v1::{
    PersistentVolumeClaim, PersistentVolumeClaimSpec, ResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Debug};

// This struct allows specifying memory and cpu limits as well as generically adding storage
// settings.
#[derive(Clone, Debug, Default, Fragment, PartialEq, JsonSchema)]
#[fragment(
    bound = "T: FromFragment, K: FromFragment",
    path_overrides(fragment = "crate::config::fragment")
)]
#[fragment_attrs(
    derive(Merge, Serialize, Deserialize, JsonSchema, Derivative),
    derivative(
        Default(bound = "T::Fragment: Default, K::Fragment: Default"),
        Debug(bound = "T::Fragment: Debug, K::Fragment: Debug"),
        Clone(bound = "T::Fragment: Clone, K::Fragment: Clone"),
        PartialEq(bound = "T::Fragment: PartialEq, K::Fragment: PartialEq")
    ),
    merge(
        bound = "T::Fragment: Merge, K::Fragment: Merge",
        path_overrides(merge = "crate::config::merge")
    ),
    serde(
        bound(
            serialize = "T::Fragment: Serialize, K::Fragment: Serialize",
            deserialize = "T::Fragment: Deserialize<'de> + Default, K::Fragment: Deserialize<'de> + Default",
        ),
        rename_all = "camelCase",
    ),
    schemars(
        bound = "T: JsonSchema, K: JsonSchema, T::Fragment: JsonSchema + Default, K::Fragment: JsonSchema + Default"
    )
)]
pub struct Resources<T, K = NoRuntimeLimits> {
    #[fragment_attrs(serde(default))]
    pub memory: MemoryLimits<K>,
    #[fragment_attrs(serde(default))]
    pub cpu: CpuLimits,
    #[fragment_attrs(serde(default))]
    pub storage: T,
}

// Defines memory limits to be set on the pods
// Is generic to enable adding custom configuration for specific runtimes or products
#[derive(Clone, Debug, Default, Fragment, PartialEq, JsonSchema)]
#[fragment(
    bound = "T: FromFragment",
    path_overrides(fragment = "crate::config::fragment")
)]
#[fragment_attrs(
    derive(Merge, Serialize, Deserialize, JsonSchema, Derivative),
    derivative(
        Default(bound = "T::Fragment: Default"),
        Debug(bound = "T::Fragment: Debug"),
        Clone(bound = "T::Fragment: Clone"),
        PartialEq(bound = "T::Fragment: PartialEq")
    ),
    merge(
        bound = "T::Fragment: Merge",
        path_overrides(merge = "crate::config::merge")
    ),
    serde(
        bound(
            serialize = "T::Fragment: Serialize",
            deserialize = "T::Fragment: Deserialize<'de> + Default",
        ),
        rename_all = "camelCase",
    ),
    schemars(bound = "T: JsonSchema, T::Fragment: JsonSchema + Default")
)]
pub struct MemoryLimits<T> {
    // The maximum amount of memory that should be available
    // Should in most cases be mapped to resources.limits.memory
    pub limit: Option<Quantity>,
    // Additional options that may be required
    #[fragment_attrs(serde(default))]
    pub runtime_limits: T,
}

// Default struct to allow operators not specifying `runtime_limits` when using [`MemoryLimits`]
#[derive(Clone, Debug, Default, Eq, Fragment, PartialEq, JsonSchema)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Clone,
        Debug,
        Default,
        Deserialize,
        Eq,
        JsonSchema,
        Merge,
        PartialEq,
        Serialize
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct NoRuntimeLimits {}

// Definition of Java Heap settings
// `min` is optional and should usually be defaulted to the same value as `max` by the implementing
// code
#[derive(Clone, Debug, Default, Fragment, PartialEq, JsonSchema)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Merge,
        Serialize,
        Deserialize,
        JsonSchema,
        Default,
        Debug,
        Clone,
        PartialEq
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct JvmHeapLimits {
    pub max: Option<Quantity>,
    #[fragment_attrs(serde(default, skip_serializing_if = "Option::is_none"))]
    pub min: Option<Quantity>,
}

// Cpu limits
// These should usually be forwarded to resources.limits.cpu
#[derive(Clone, Debug, Default, Fragment, PartialEq, JsonSchema)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Merge,
        Serialize,
        Deserialize,
        JsonSchema,
        Default,
        Debug,
        Clone,
        PartialEq
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct CpuLimits {
    pub min: Option<Quantity>,
    pub max: Option<Quantity>,
}

// Struct that exposes the values for a PVC which the user should be able to influence
#[derive(Clone, Debug, Default, Fragment, PartialEq, JsonSchema)]
#[fragment(path_overrides(fragment = "crate::config::fragment"))]
#[fragment_attrs(
    derive(
        Merge,
        Serialize,
        Deserialize,
        JsonSchema,
        Default,
        Debug,
        Clone,
        PartialEq
    ),
    merge(path_overrides(merge = "crate::config::merge")),
    serde(rename_all = "camelCase")
)]
pub struct PvcConfig {
    pub capacity: Option<Quantity>,
    #[fragment_attrs(serde(default, skip_serializing_if = "Option::is_none"))]
    pub storage_class: Option<String>,
    #[fragment_attrs(serde(default, skip_serializing_if = "Option::is_none"))]
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
                selector: self.selectors.clone(),
                storage_class_name: self.storage_class.clone(),
                resources: Some(ResourceRequirements {
                    requests: Some({
                        let mut map = BTreeMap::new();
                        if let Some(capacity) = &self.capacity {
                            map.insert("storage".to_string(), capacity.clone());
                        }
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
    T: Clone + Default,
    K: Clone + Default,
{
    fn into(self) -> ResourceRequirements {
        let mut limits = BTreeMap::new();
        let mut requests = BTreeMap::new();
        if let Some(memory_limit) = self.memory.limit {
            limits.insert("memory".to_string(), memory_limit.clone());
            requests.insert("memory".to_string(), memory_limit);
        }

        if let Some(cpu_max) = self.cpu.max {
            limits.insert("cpu".to_string(), cpu_max);
        }
        if let Some(cpu_min) = self.cpu.min {
            requests.insert("cpu".to_string(), cpu_min);
        }

        ResourceRequirements {
            limits: if limits.is_empty() {
                None
            } else {
                Some(limits)
            },
            requests: if requests.is_empty() {
                None
            } else {
                Some(requests)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::commons::resources::{PvcConfig, PvcConfigFragment, Resources, ResourcesFragment};
    use crate::config::{
        fragment::{self, Fragment},
        merge::Merge,
    };
    use k8s_openapi::api::core::v1::{PersistentVolumeClaim, ResourceRequirements};
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, Fragment)]
    #[fragment(path_overrides(fragment = "crate::config::fragment"))]
    #[fragment_attrs(
        derive(Serialize, Deserialize, Merge, Default),
        merge(path_overrides(merge = "crate::config::merge"))
    )]
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
            storageClassName: CustomClass
            resources:
                requests:
                    storage: 10Gi"#
    )]
    #[case::selector(
        "test",
        None,
        r#"
        capacity: 10Gi
        storageClass: CustomClass
        selectors:
            matchLabels:
                nodeType: directstorage"#,
        r#"
        apiVersion: v1
        kind: PersistentVolumeClaim
        metadata:
            name: test
        spec:
            storageClassName: CustomClass
            resources:
                requests:
                    storage: 10Gi
            selector:
                matchLabels:
                    nodeType: directstorage"#
    )]
    fn test_build_pvc(
        #[case] name: String,
        #[case] access_modes: Option<Vec<&str>>,
        #[case] input: String,
        #[case] expected: String,
    ) {
        let input_pvcconfig_fragment: PvcConfigFragment =
            serde_yaml::from_str(&input).expect("illegal test input");
        let input_pvcconfig = fragment::validate::<PvcConfig>(input_pvcconfig_fragment)
            .expect("test input failed validation");

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
        let input_resources_fragment: ResourcesFragment<TestStorageConfig> =
            serde_yaml::from_str(&input).expect("illegal test input");
        let input_resources: Resources<TestStorageConfig> =
            fragment::validate(input_resources_fragment).expect("test input failed validation");

        let result: ResourceRequirements = input_resources.into();
        let expected_requirements: ResourceRequirements =
            serde_yaml::from_str(&expected).expect("illegal expected output");

        assert_eq!(result, expected_requirements);
    }
}
