use std::fs::create_dir_all;

use snafu::{Report, ResultExt, Snafu};
use stackable_operator::{
    YamlSchema,
    commons::resources::{JvmHeapLimits, Resources},
    config::fragment::Fragment,
    crd::{
        authentication::core::AuthenticationClass,
        listener::{Listener, ListenerClass, PodListeners},
        s3::{S3Bucket, S3Connection},
    },
    k8s_openapi::serde::{Deserialize, Serialize},
    kube::{CustomResource, core::crd::MergeError},
    role_utils::Role,
    schemars::JsonSchema,
    shared::yaml::SerializeOptions,
    status::condition::ClusterCondition,
};
use stackable_versioned::versioned;

const OPERATOR_VERSION: &str = "0.0.0-dev";
const OUTPUT_DIR: &str = "../../generated-crd-previews";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to merge CRD for CRD {crd}"))]
    MergeCRD { source: MergeError, crd: String },

    #[snafu(display("Failed to create output directory {dir}"))]
    CreateOutputDir { source: std::io::Error, dir: String },

    #[snafu(display("Failed to write CRD to output file"))]
    WriteCRD {
        source: stackable_shared::yaml::Error,
    },
}

pub fn main() -> Report<Error> {
    Report::capture(write_crds)
}

macro_rules! write_crd {
    ($crd_name:ident, $stored_crd_version:ident) => {
        $crd_name::merged_crd($crd_name::$stored_crd_version)
            .with_context(|_| MergeCRDSnafu {
                crd: stringify!($crd_name),
            })?
            .write_yaml_schema(
                format!("{OUTPUT_DIR}/{}.yaml", stringify!($crd_name)),
                OPERATOR_VERSION,
                SerializeOptions::default(),
            )
            .context(WriteCRDSnafu)?;
    };
}

pub fn write_crds() -> Result<(), Error> {
    create_dir_all(OUTPUT_DIR).with_context(|_| CreateOutputDirSnafu {
        dir: OUTPUT_DIR.to_string(),
    })?;

    // AuthenticationClass::merged_crd(AuthenticationClass::V1Alpha1)
    //     .with_context(|_| MergeCRDSnafu {
    //         crd: "AuthenticationClass".to_string(),
    //     })?
    //     .write_yaml_schema(
    //         format!("{OUTPUT_DIR}/{}.yaml", "AuthenticationClass"),
    //         OPERATOR_VERSION,
    //         SerializeOptions::default(),
    //     )
    //     .context(WriteCRDSnafu)?;

    write_crd!(AuthenticationClass, V1Alpha1);
    write_crd!(Listener, V1Alpha1);
    write_crd!(ListenerClass, V1Alpha1);
    write_crd!(PodListeners, V1Alpha1);
    write_crd!(S3Bucket, V1Alpha1);
    write_crd!(S3Connection, V1Alpha1);

    // Also write a CRD with all sorts of common structs
    write_crd!(DummyCluster, V1Alpha1);

    Ok(())
}

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {

    #[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[versioned(k8s(
        group = "zookeeper.stackable.tech",
        kind = "DummyCluster",
        status = "v1alpha1::DummyClusterStatus",
        namespaced,
        crates(
            kube_core = "stackable_operator::kube::core",
            k8s_openapi = "stackable_operator::k8s_openapi",
            schemars = "stackable_operator::schemars"
        )
    ))]
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
        serde(rename_all = "camelCase")
    )]
    pub struct ProductConfig {
        #[fragment_attrs(serde(default))]
        resources: Resources<ProductStorageConfig, JvmHeapLimits>,
    }

    #[derive(Debug, Default, PartialEq, Fragment, JsonSchema)]
    #[fragment_attrs(
        derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema),
        serde(rename_all = "camelCase")
    )]
    pub struct ProductStorageConfig {
        data_storage: stackable_operator::commons::resources::PvcConfig,
    }

    #[derive(Clone, Default, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DummyClusterStatus {
        pub conditions: Vec<ClusterCondition>,
    }
}
