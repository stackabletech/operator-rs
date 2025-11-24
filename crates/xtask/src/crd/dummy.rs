use serde::{Deserialize, Serialize};
use stackable_operator::{
    commons::resources::{JvmHeapLimits, Resources},
    config::fragment::Fragment,
    deep_merger::ObjectOverrides,
    kube::CustomResource,
    role_utils::Role,
    schemars::JsonSchema,
    status::condition::ClusterCondition,
    versioned::versioned,
};
use strum::{Display, EnumIter};

#[versioned(
    version(name = "v1alpha1"),
    crates(
        kube_core = "stackable_operator::kube::core",
        kube_client = "stackable_operator::kube::client",
        k8s_openapi = "stackable_operator::k8s_openapi",
        schemars = "stackable_operator::schemars",
        versioned = "stackable_operator::versioned"
    )
)]
pub mod versioned {
    #[versioned(crd(
        group = "dummy.stackable.tech",
        kind = "DummyCluster",
        status = "v1alpha1::DummyClusterStatus",
        namespaced,
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
        secret_reference: stackable_operator::shared::secret::SecretReference,
        tls_client_details: stackable_operator::commons::tls_verification::TlsClientDetails,

        #[serde(flatten)]
        pub object_overrides: ObjectOverrides,

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

        #[fragment_attrs(serde(default))]
        pub logging: stackable_operator::product_logging::spec::Logging<Container>,
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

    #[derive(
        Clone,
        Debug,
        Deserialize,
        Display,
        Eq,
        EnumIter,
        JsonSchema,
        Ord,
        PartialEq,
        PartialOrd,
        Serialize,
    )]
    #[serde(rename_all = "kebab-case")]
    #[strum(serialize_all = "kebab-case")]
    #[schemars(crate = "stackable_operator::schemars")]
    pub enum Container {
        Prepare,
        Vector,
        BundleBuilder,
        Opa,
        UserInfoFetcher,
    }
    #[derive(Clone, Default, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[schemars(crate = "stackable_operator::schemars")]
    #[serde(rename_all = "camelCase")]
    pub struct DummyClusterStatus {
        pub conditions: Vec<ClusterCondition>,
    }
}
