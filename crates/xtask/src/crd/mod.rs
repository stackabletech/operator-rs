use std::path::PathBuf;

use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::{
    crd::{
        authentication::core::AuthenticationClass,
        listener::{Listener, ListenerClass, PodListeners},
        s3::{S3Bucket, S3Connection},
    },
    kube::core::crd::MergeError,
};

use crate::crd::dummy::DummyCluster;

mod dummy;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to get manifest directory"))]
    GetManifestDirectory { source: std::env::VarError },

    #[snafu(display("failed to get parent directory of {path}", path = path.display()))]
    GetParentDirectory { path: PathBuf },

    #[snafu(display("failed to merge {crd_name:?} CRD"))]
    MergeCrd {
        source: MergeError,
        crd_name: String,
    },

    #[snafu(display("failed to write CRD to file at {path}", path = path.display()))]
    WriteCrd {
        source: stackable_operator::shared::yaml::Error,
        path: PathBuf,
    },
}

macro_rules! write_crd {
    ($base_path:expr, $crd_name:ident, $stored_crd_version:ident) => {
        let merged = $crd_name::merged_crd($crd_name::$stored_crd_version)
            .context(MergeCrdSnafu { crd_name: stringify!($crd_name) })?;

        let mut path = $base_path.join(stringify!($crd_name));
        path.set_extension("yaml");

        <stackable_operator::k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition
            as stackable_operator::YamlSchema>
            ::write_yaml_schema(
                &merged,
                &path,
                "0.0.0-dev",
                stackable_operator::shared::yaml::SerializeOptions::default(),
            )
            .with_context(|_| WriteCrdSnafu { path: path.clone() })?;
    };
}

pub fn generate_preview() -> Result<(), Error> {
    let path = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .context(GetManifestDirectorySnafu)?;

    let path = path
        .parent()
        .with_context(|| GetParentDirectorySnafu { path: path.clone() })?
        .join("stackable-operator/crds");

    write_crd!(path, AuthenticationClass, V1Alpha1);
    write_crd!(path, Listener, V1Alpha1);
    write_crd!(path, ListenerClass, V1Alpha1);
    write_crd!(path, PodListeners, V1Alpha1);
    write_crd!(path, S3Bucket, V1Alpha1);
    write_crd!(path, S3Connection, V1Alpha1);

    write_crd!(path, DummyCluster, V1Alpha1);

    Ok(())
}
