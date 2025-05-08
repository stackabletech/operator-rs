use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    commons::{
        networking::HostName, secret_class::SecretClassVolume, tls_verification::TlsClientDetails,
    },
    versioned::versioned,
};

mod v1alpha1_impl;

// FIXME (@Techassi): This should be versioned as well, but the macro cannot
// handle new-type structs yet.
/// Use this type in you operator!
pub type ResolvedConnection = v1alpha1::ConnectionSpec;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    pub mod v1alpha1 {
        pub use v1alpha1_impl::ConnectionError;
    }

    /// S3 connection definition as a resource.
    /// Learn more on the [S3 concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/s3).
    #[versioned(k8s(
        group = "s3.stackable.tech",
        kind = "S3Connection",
        plural = "s3connections",
        crates(
            kube_core = "kube::core",
            k8s_openapi = "k8s_openapi",
            schemars = "schemars"
        ),
        namespaced
    ))]
    #[derive(CustomResource, Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ConnectionSpec {
        /// Host of the S3 server without any protocol or port. For example: `west1.my-cloud.com`.
        pub host: HostName,

        /// Port the S3 server listens on.
        /// If not specified the product will determine the port to use.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub port: Option<u16>,

        /// Bucket region used for signing headers (sigv4).
        ///
        /// This defaults to `us-east-1` which is compatible with other implementations such as Minio.
        ///
        /// WARNING: Some products use the Hadoop S3 implementation which falls back to us-east-2.
        #[serde(default)]
        pub region: Region,

        /// Which access style to use.
        /// Defaults to virtual hosted-style as most of the data products out there.
        /// Have a look at the [AWS documentation](https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html).
        #[serde(default)]
        pub access_style: S3AccessStyle,

        /// If the S3 uses authentication you have to specify you S3 credentials.
        /// In the most cases a [SecretClass](DOCS_BASE_URL_PLACEHOLDER/secret-operator/secretclass)
        /// providing `accessKey` and `secretKey` is sufficient.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub credentials: Option<SecretClassVolume>,

        /// Use a TLS connection. If not specified no TLS will be used.
        #[serde(flatten)]
        pub tls: TlsClientDetails,
    }

    #[derive(
        strum::Display, Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize,
    )]
    #[strum(serialize_all = "PascalCase")]
    pub enum S3AccessStyle {
        /// Use path-style access as described in <https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html#path-style-access>
        Path,

        /// Use as virtual hosted-style access as described in <https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html#virtual-hosted-style-access>
        #[default]
        VirtualHosted,
    }

    /// Set a named S3 Bucket region.
    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Region {
        #[serde(default = "v1alpha1::Region::default_region_name")]
        pub name: String,
    }

    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    // TODO: This probably should be serde(untagged), but this would be a breaking change
    pub enum InlineConnectionOrReference {
        Inline(ConnectionSpec),
        Reference(String),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use url::Url;

    use super::*;
    use crate::commons::{
        secret_class::SecretClassVolume,
        tls_verification::{CaCert, Tls, TlsClientDetails, TlsServerVerification, TlsVerification},
    };

    // We cant test the correct resolve, as we can't mock the k8s API.
    #[test]
    fn http_endpoint() {
        let s3 = ResolvedConnection {
            host: "minio".parse().unwrap(),
            port: None,
            access_style: Default::default(),
            credentials: None,
            tls: TlsClientDetails { tls: None },
            region: Default::default(),
        };
        let (volumes, mounts) = s3.volumes_and_mounts().unwrap();

        assert_eq!(s3.endpoint().unwrap(), Url::parse("http://minio").unwrap());
        assert_eq!(volumes, vec![]);
        assert_eq!(mounts, vec![]);
    }

    #[test]
    fn https_endpoint() {
        let s3 = ResolvedConnection {
            host: "s3-eu-central-2.ionoscloud.com".parse().unwrap(),
            port: None,
            access_style: Default::default(),
            credentials: Some(SecretClassVolume {
                secret_class: "ionos-s3-credentials".to_string(),
                scope: None,
            }),
            tls: TlsClientDetails {
                tls: Some(Tls {
                    verification: TlsVerification::Server(TlsServerVerification {
                        ca_cert: CaCert::WebPki {},
                    }),
                }),
            },
            region: Default::default(),
        };
        let (mut volumes, mut mounts) = s3.volumes_and_mounts().unwrap();

        assert_eq!(
            s3.endpoint().unwrap(),
            Url::parse("https://s3-eu-central-2.ionoscloud.com").unwrap()
        );
        assert_eq!(volumes.len(), 1);
        let volume = volumes.remove(0);
        assert_eq!(mounts.len(), 1);
        let mount = mounts.remove(0);

        assert_eq!(&volume.name, "ionos-s3-credentials-s3-credentials");
        assert_eq!(
            &volume
                .ephemeral
                .unwrap()
                .volume_claim_template
                .unwrap()
                .metadata
                .unwrap()
                .annotations
                .unwrap(),
            &BTreeMap::from([(
                "secrets.stackable.tech/class".to_string(),
                "ionos-s3-credentials".to_string()
            )]),
        );

        assert_eq!(mount.name, volume.name);
        assert_eq!(mount.mount_path, "/stackable/secrets/ionos-s3-credentials");
        assert_eq!(
            s3.credentials_mount_paths(),
            Some((
                "/stackable/secrets/ionos-s3-credentials/accessKey".to_string(),
                "/stackable/secrets/ionos-s3-credentials/secretKey".to_string()
            ))
        );
    }

    #[test]
    fn https_without_verification() {
        let s3 = ResolvedConnection {
            host: "minio".parse().unwrap(),
            port: Some(1234),
            access_style: Default::default(),
            credentials: None,
            tls: TlsClientDetails {
                tls: Some(Tls {
                    verification: TlsVerification::None {},
                }),
            },
            region: Default::default(),
        };
        let (volumes, mounts) = s3.volumes_and_mounts().unwrap();

        assert_eq!(
            s3.endpoint().unwrap(),
            Url::parse("https://minio:1234").unwrap()
        );
        assert_eq!(volumes, vec![]);
        assert_eq!(mounts, vec![]);
    }
}
