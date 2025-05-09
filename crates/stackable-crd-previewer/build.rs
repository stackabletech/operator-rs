use std::fs::create_dir_all;

use snafu::{Report, ResultExt, Snafu};
use stackable_operator::{
    YamlSchema,
    crd::{
        authentication::core::AuthenticationClass,
        listener::{Listener, ListenerClass, PodListeners},
        s3::{S3Bucket, S3Connection},
    },
    kube::core::crd::MergeError,
    shared::yaml::SerializeOptions,
};

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

pub fn main() -> Report<Error> {
    Report::capture(|| write_crds())
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

    Ok(())
}
