use serde::{Deserialize, Serialize};
use stackable_operator::{
    commons::resources::{JvmHeapLimits, Resources},
    config::fragment::Fragment,
    kube::CustomResource,
    role_utils::Role,
    schemars::JsonSchema,
    status::condition::ClusterCondition,
    versioned::versioned,
};

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    #[versioned(k8s(
        group = "dummy.stackable.tech",
        kind = "DummyCluster",
        status = "v1alpha1::DummyClusterStatus",
        namespaced,
        crates(
            kube_core = "stackable_operator::kube::core",
            kube_client = "stackable_operator::kube::client",
            k8s_openapi = "stackable_operator::k8s_openapi",
            schemars = "stackable_operator::schemars",
            versioned = "stackable_operator::versioned"
        )
    ))]
    #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[schemars(crate = "stackable_operator::schemars")]
    #[serde(rename_all = "camelCase")]
    pub struct DummyClusterSpec {
        nodes: Option<Role<ProductConfigFragment>>,

        // Not versioned yet
        stackable_affinity: stackable_operator::commons::affinity::StackableAffinity,
        stackable_node_selector: stackable_operator::commons::affinity::StackableNodeSelector,
        user_information_cache: stackable_operator::commons::cache::UserInformationCache,
        cluster_operation: stackable_operator::commons::cluster_operation::ClusterOperation,
        domain_name: stackable_operator::commons::networking::DomainName,
        host_name: stackable_operator::commons::networking::HostName,
        kerberos_realm_name: stackable_operator::commons::networking::KerberosRealmName,
        opa_config: stackable_operator::commons::opa::OpaConfig,
        pdb_config: stackable_operator::commons::pdb::PdbConfig,
        product_image: stackable_operator::commons::product_image_selection::ProductImage,
        secret_class_volume: stackable_operator::commons::secret_class::SecretClassVolume,
        secret_reference: stackable_operator::commons::secret::SecretReference,
        tls_client_details: stackable_operator::commons::tls_verification::TlsClientDetails,

        // Already versioned
        client_authentication_details:
            stackable_operator::crd::authentication::core::v1alpha1::ClientAuthenticationDetails,
    }

    #[derive(Debug, Default, PartialEq, Fragment, JsonSchema)]
    #[fragment_attrs(
        derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema),
        schemars(crate = "stackable_operator::schemars"),
        serde(rename_all = "camelCase")
    )]
    #[schemars(crate = "stackable_operator::schemars")]
    pub struct ProductConfig {
        #[fragment_attrs(serde(default))]
        resources: Resources<ProductStorageConfig, JvmHeapLimits>,
    }

    #[derive(Debug, Default, PartialEq, Fragment, JsonSchema)]
    #[fragment_attrs(
        derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema),
        schemars(crate = "stackable_operator::schemars"),
        serde(rename_all = "camelCase")
    )]
    #[schemars(crate = "stackable_operator::schemars")]
    pub struct ProductStorageConfig {
        data_storage: stackable_operator::commons::resources::PvcConfig,
    }

    #[derive(Clone, Default, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[schemars(crate = "stackable_operator::schemars")]
    #[serde(rename_all = "camelCase")]
    pub struct DummyClusterStatus {
        pub conditions: Vec<ClusterCondition>,
    }
}
