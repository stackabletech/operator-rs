use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt as _, Snafu};
use stackable_versioned::versioned;
use url::Url;

use crate::{
    builder::pod::{PodBuilder, container::ContainerBuilder, volume::VolumeMountBuilder},
    commons::{
        networking::HostName,
        secret_class::{SecretClassVolume, SecretClassVolumeError},
        tls_verification::{TlsClientDetails, TlsClientDetailsError},
    },
    constants::secret::SECRET_BASE_PATH,
    k8s_openapi::api::core::v1::{Volume, VolumeMount},
};

mod v1alpha1_impl;

// NOTE (@Techassi): Where should this error be placed? Technically errors can
// change between version., because version-specific impl blocks might need
// different variants or might use a completely different error type.
#[derive(Debug, Snafu)]
pub enum ConnectionError {
    #[snafu(display("failed to retrieve S3 connection '{s3_connection}'"))]
    RetrieveS3Connection {
        source: crate::client::Error,
        s3_connection: String,
    },

    #[snafu(display("failed to parse S3 endpoint '{endpoint}'"))]
    ParseS3Endpoint {
        source: url::ParseError,
        endpoint: String,
    },

    #[snafu(display("failed to set S3 endpoint scheme '{scheme}' for endpoint '{endpoint}'"))]
    SetS3EndpointScheme { endpoint: Url, scheme: String },

    #[snafu(display("failed to add S3 credential volumes and volume mounts"))]
    AddS3CredentialVolumes { source: SecretClassVolumeError },

    #[snafu(display("failed to add S3 TLS client details volumes and volume mounts"))]
    AddS3TlsClientDetailsVolumes { source: TlsClientDetailsError },

    #[snafu(display("failed to add required volumes"))]
    AddVolumes { source: crate::builder::pod::Error },

    #[snafu(display("failed to add required volumeMounts"))]
    AddVolumeMounts {
        source: crate::builder::pod::container::Error,
    },
}

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
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

// FIXME (@Techassi): This should be versioned as well, but the macro cannot
// handle new-type structs yet.
/// Use this type in you operator!
pub type ResolvedConnection = v1alpha1::ConnectionSpec;

impl ResolvedConnection {
    /// Build the endpoint URL from this connection
    pub fn endpoint(&self) -> Result<Url, ConnectionError> {
        let endpoint = format!(
            "http://{host}:{port}",
            host = self.host.as_url_host(),
            port = self.port()
        );
        let mut url = Url::parse(&endpoint).context(ParseS3EndpointSnafu { endpoint })?;

        if self.tls.uses_tls() {
            url.set_scheme("https").map_err(|_| {
                SetS3EndpointSchemeSnafu {
                    scheme: "https".to_string(),
                    endpoint: url.clone(),
                }
                .build()
            })?;
        }

        Ok(url)
    }

    /// Returns the port to be used, which is either user configured or defaulted based upon TLS usage
    pub fn port(&self) -> u16 {
        self.port
            .unwrap_or(if self.tls.uses_tls() { 443 } else { 80 })
    }

    /// This functions adds
    ///
    /// * Credentials needed to connect to S3
    /// * Needed TLS volumes
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) -> Result<(), ConnectionError> {
        let (volumes, mounts) = self.volumes_and_mounts()?;
        pod_builder.add_volumes(volumes).context(AddVolumesSnafu)?;
        for cb in container_builders {
            cb.add_volume_mounts(mounts.clone())
                .context(AddVolumeMountsSnafu)?;
        }

        Ok(())
    }

    /// It is recommended to use [`Self::add_volumes_and_mounts`], this function returns you the
    /// volumes and mounts in case you need to add them by yourself.
    pub fn volumes_and_mounts(&self) -> Result<(Vec<Volume>, Vec<VolumeMount>), ConnectionError> {
        let mut volumes = Vec::new();
        let mut mounts = Vec::new();

        if let Some(credentials) = &self.credentials {
            let secret_class = &credentials.secret_class;
            let volume_name = format!("{secret_class}-s3-credentials");

            volumes.push(
                credentials
                    .to_volume(&volume_name)
                    .context(AddS3CredentialVolumesSnafu)?,
            );
            mounts.push(
                VolumeMountBuilder::new(volume_name, format!("{SECRET_BASE_PATH}/{secret_class}"))
                    .build(),
            );
        }

        // Add needed TLS volumes
        let (tls_volumes, tls_mounts) = self
            .tls
            .volumes_and_mounts()
            .context(AddS3TlsClientDetailsVolumesSnafu)?;
        volumes.extend(tls_volumes);
        mounts.extend(tls_mounts);

        Ok((volumes, mounts))
    }

    /// Returns the path of the files containing bind user and password.
    /// This will be None if there are no credentials for this LDAP connection.
    pub fn credentials_mount_paths(&self) -> Option<(String, String)> {
        self.credentials.as_ref().map(|bind_credentials| {
            let secret_class = &bind_credentials.secret_class;
            (
                format!("{SECRET_BASE_PATH}/{secret_class}/accessKey"),
                format!("{SECRET_BASE_PATH}/{secret_class}/secretKey"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

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
