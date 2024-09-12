use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use url::Url;

use crate::{
    builder::pod::{container::ContainerBuilder, volume::VolumeMountBuilder, PodBuilder},
    client::Client,
    commons::authentication::SECRET_BASE_PATH,
};

use super::{
    AddS3CredentialVolumesSnafu, AddS3TlsClientDetailsVolumesSnafu, ParseS3EndpointSnafu,
    RetrieveS3ConnectionSnafu, S3Bucket, S3BucketSpec, S3Connection, S3ConnectionSpec, S3Error,
    SetS3EndpointSchemeSnafu,
};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
// TODO: This probably should be serde(untagged), but this would be a breaking change
pub enum S3ConnectionInlineOrReference {
    Inline(S3ConnectionSpec),
    Reference(String),
}

/// Use this type in you operator!
pub type ResolvedS3Connection = S3ConnectionSpec;

impl S3ConnectionInlineOrReference {
    pub async fn resolve(
        self,
        client: &Client,
        namespace: &str,
    ) -> Result<ResolvedS3Connection, S3Error> {
        match self {
            Self::Inline(inline) => Ok(inline),
            Self::Reference(reference) => Ok(client
                .get::<S3Connection>(&reference, namespace)
                .await
                .context(RetrieveS3ConnectionSnafu)?
                .spec),
        }
    }
}

impl ResolvedS3Connection {
    /// Build the endpoint URL from this connection
    pub fn endpoint(&self) -> Result<Url, S3Error> {
        let mut url = Url::parse(&format!(
            "http://{}:{}",
            self.host.as_url_host(),
            self.port()
        ))
        .context(ParseS3EndpointSnafu)?;

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
        unique_identifier: &str,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) -> Result<(), S3Error> {
        let (volumes, mounts) = self.volumes_and_mounts(unique_identifier)?;
        pod_builder.add_volumes(volumes);
        for cb in container_builders {
            cb.add_volume_mounts(mounts.clone());
        }

        Ok(())
    }

    /// It is recommended to use [`Self::add_volumes_and_mounts`], this function returns you the
    /// volumes and mounts in case you need to add them by yourself.
    pub fn volumes_and_mounts(
        &self,
        unique_identifier: &str,
    ) -> Result<(Vec<Volume>, Vec<VolumeMount>), S3Error> {
        let mut volumes = Vec::new();
        let mut mounts = Vec::new();

        if let Some(credentials) = &self.credentials {
            let secret_class = &credentials.secret_class;
            let volume_name = format!("{secret_class}-s3-credentials-{unique_identifier}");

            volumes.push(
                credentials
                    .to_volume(&volume_name)
                    .context(AddS3CredentialVolumesSnafu)?,
            );
            mounts.push(
                VolumeMountBuilder::new(
                    volume_name,
                    format!("{SECRET_BASE_PATH}/{secret_class}-{unique_identifier}"),
                )
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
    pub fn credentials_mount_paths(&self, unique_identifier: &str) -> Option<(String, String)> {
        self.credentials.as_ref().map(|bind_credentials| {
            let secret_class = &bind_credentials.secret_class;
            (
                format!("{SECRET_BASE_PATH}/{secret_class}-{unique_identifier}/accessKey"),
                format!("{SECRET_BASE_PATH}/{secret_class}-{unique_identifier}/secretKey"),
            )
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
// TODO: This probably should be serde(untagged), but this would be a breaking change
pub enum S3BucketInlineOrReference {
    Inline(S3BucketSpec),
    Reference(String),
}

/// Use this struct in your operator.
pub struct ResolvedS3Bucket {
    pub bucket_name: String,
    pub connection: S3ConnectionSpec,
}

impl S3BucketInlineOrReference {
    pub async fn resolve(
        self,
        client: &Client,
        namespace: &str,
    ) -> Result<ResolvedS3Bucket, S3Error> {
        match self {
            Self::Inline(inline) => Ok(ResolvedS3Bucket {
                bucket_name: inline.bucket_name,
                connection: inline.connection.resolve(client, namespace).await?,
            }),
            Self::Reference(reference) => {
                let bucket = client
                    .get::<S3Bucket>(&reference, namespace)
                    .await
                    .context(RetrieveS3ConnectionSnafu)?
                    .spec;
                Ok(ResolvedS3Bucket {
                    bucket_name: bucket.bucket_name,
                    connection: bucket.connection.resolve(client, namespace).await?,
                })
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use crate::commons::{
        secret_class::SecretClassVolume,
        tls_verification::{CaCert, Tls, TlsClientDetails, TlsServerVerification, TlsVerification},
    };

    use super::*;

    // We cant test the correct resolve, as we can't mock the k8s API.

    #[test]
    fn test_http() {
        let s3 = ResolvedS3Connection {
            host: "minio".parse().unwrap(),
            port: None,
            access_style: Default::default(),
            credentials: None,
            tls: TlsClientDetails { tls: None },
        };
        let (volumes, mounts) = s3.volumes_and_mounts("lakehouse").unwrap();

        assert_eq!(s3.endpoint().unwrap(), Url::parse("http://minio").unwrap());
        assert_eq!(volumes, vec![]);
        assert_eq!(mounts, vec![]);
    }

    #[test]
    fn test_https() {
        let s3 = ResolvedS3Connection {
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
        };
        let (mut volumes, mut mounts) = s3.volumes_and_mounts("lakehouse").unwrap();

        assert_eq!(
            s3.endpoint().unwrap(),
            Url::parse("https://s3-eu-central-2.ionoscloud.com").unwrap()
        );
        assert_eq!(volumes.len(), 1);
        let volume = volumes.remove(0);
        assert_eq!(mounts.len(), 1);
        let mount = mounts.remove(0);

        assert_eq!(
            &volume.name,
            "ionos-s3-credentials-s3-credentials-lakehouse"
        );
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
        assert_eq!(
            mount.mount_path,
            "/stackable/secrets/ionos-s3-credentials-lakehouse"
        );
        assert_eq!(
            s3.credentials_mount_paths("lakehouse"),
            Some((
                "/stackable/secrets/ionos-s3-credentials-lakehouse/accessKey".to_string(),
                "/stackable/secrets/ionos-s3-credentials-lakehouse/secretKey".to_string()
            ))
        );
    }

    #[test]
    fn test_https_without_verification() {
        let s3 = ResolvedS3Connection {
            host: "minio".parse().unwrap(),
            port: Some(1234),
            access_style: Default::default(),
            credentials: None,
            tls: TlsClientDetails {
                tls: Some(Tls {
                    verification: TlsVerification::None {},
                }),
            },
        };
        let (volumes, mounts) = s3.volumes_and_mounts("lakehouse").unwrap();

        assert_eq!(
            s3.endpoint().unwrap(),
            Url::parse("https://minio:1234").unwrap()
        );
        assert_eq!(volumes, vec![]);
        assert_eq!(mounts, vec![]);
    }
}
