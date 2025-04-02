//! This module contains resource types to interact with [`v1alpha1::ListenerClass`]es.
//!
//! It declares a policy for how [`v1alpha1::Listener`][listener]s are exposed to users. It is
//! created by the cluster administrator.
//!
//! [listener]: crate::crd::listener::listeners::v1alpha1::Listener

use std::collections::BTreeMap;

#[cfg(doc)]
use k8s_openapi::api::core::v1::Service;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

use crate::crd::listener::core::v1alpha1 as core_v1alpha1;
#[cfg(doc)]
use crate::crd::listener::listeners::v1alpha1::Listener;

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// Defines a policy for how [Listeners](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listener) should be exposed.
    /// Read the [ListenerClass documentation](DOCS_BASE_URL_PLACEHOLDER/listener-operator/listenerclass)
    /// for more information.
    #[versioned(k8s(group = "listeners.stackable.tech"))]
    #[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct ListenerClassSpec {
        pub service_type: core_v1alpha1::ServiceType,

        /// Annotations that should be added to the Service object.
        #[serde(default)]
        pub service_annotations: BTreeMap<String, String>,

        /// `externalTrafficPolicy` that should be set on the created [`Service`] objects.
        ///
        /// The default is `Local` (in contrast to `Cluster`), as we aim to direct traffic to a node running the workload
        /// and we should keep testing that as the primary configuration. Cluster is a fallback option for providers that
        /// break Local mode (IONOS so far).
        #[serde(default = "ListenerClassSpec::default_service_external_traffic_policy")]
        pub service_external_traffic_policy: core_v1alpha1::KubernetesTrafficPolicy,

        /// Whether addresses should prefer using the IP address (`IP`) or the hostname (`Hostname`).
        /// Can also be set to `HostnameConservative`, which will use `IP` for `NodePort` service types, but `Hostname` for everything else.
        ///
        /// The other type will be used if the preferred type is not available.
        ///
        /// Defaults to `HostnameConservative`.
        #[serde(default = "ListenerClassSpec::default_preferred_address_type")]
        pub preferred_address_type: core_v1alpha1::PreferredAddressType,
    }
}
