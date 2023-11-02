use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use url::Url;

use crate::{
    builder::{ContainerBuilder, PodBuilder, VolumeMountBuilder},
    client::Client,
    commons::{
        s3::{
            ParseS3EndpointSnafu, RetrieveS3ConnectionSnafu, S3Bucket, S3BucketSpec, S3Connection,
            S3ConnectionSpec, S3Result, SetS3EndpointSchemeSnafu,
        },
        tls::SECRET_BASE_PATH,
    },
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
    pub async fn resolve(self, client: &Client, namespace: &str) -> S3Result<ResolvedS3Connection> {
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
    pub fn endpoint(&self) -> S3Result<Url> {
        let mut url = Url::parse(&format!("http://{}:{}", self.hostname, self.port()))
            .context(ParseS3EndpointSnafu)?;

        if self.tls.use_tls() {
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
            .unwrap_or(if self.tls.use_tls() { 443 } else { 80 })
    }

    /// This functions adds
    ///
    /// * Credentials needed to connect to S3
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) {
        let (volumes, mounts) = self.volumes_and_mounts();
        pod_builder.add_volumes(volumes);
        for cb in container_builders {
            cb.add_volume_mounts(mounts.clone());
        }
    }

    /// It is recommended to use [`Self::add_volumes_and_mounts`], this function returns you the
    /// volumes and mounts in case you need to add them by yourself.
    pub fn volumes_and_mounts(&self) -> (Vec<Volume>, Vec<VolumeMount>) {
        let mut volumes = Vec::new();
        let mut mounts = Vec::new();

        if let Some(credentials) = &self.credentials {
            let secret_class = &credentials.secret_class;
            let volume_name = format!("{secret_class}-s3-credentials");

            volumes.push(credentials.to_volume(&volume_name));
            mounts.push(
                VolumeMountBuilder::new(volume_name, format!("{SECRET_BASE_PATH}/{secret_class}"))
                    .build(),
            );
        }

        (volumes, mounts)
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
    pub async fn resolve(self, client: &Client, namespace: &str) -> S3Result<ResolvedS3Bucket> {
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
        tls::{CaCert, Tls, TlsClientDetails, TlsServerVerification, TlsVerification},
    };

    use super::*;

    // We cant test the correct resolve, as we can't mock the k8s API.

    #[test]
    fn test_http() {
        let s3 = ResolvedS3Connection {
            hostname: "minio".to_string(),
            port: None,
            access_style: Default::default(),
            credentials: None,
            tls: TlsClientDetails { tls: None },
        };
        let (volumes, mounts) = s3.volumes_and_mounts();

        assert_eq!(s3.endpoint().unwrap(), Url::parse("http://minio").unwrap());
        assert_eq!(volumes, vec![]);
        assert_eq!(mounts, vec![]);
    }

    #[test]
    fn test_https() {
        let s3 = ResolvedS3Connection {
            hostname: "s3-eu-central-2.ionoscloud.com".to_string(),
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
        let (mut volumes, mut mounts) = s3.volumes_and_mounts();

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
    fn test_https_without_verification() {
        let s3 = ResolvedS3Connection {
            hostname: "minio".to_string(),
            port: Some(1234),
            access_style: Default::default(),
            credentials: None,
            tls: TlsClientDetails {
                tls: Some(Tls {
                    verification: crate::commons::tls::TlsVerification::None {},
                }),
            },
        };
        let (volumes, mounts) = s3.volumes_and_mounts();

        assert_eq!(
            s3.endpoint().unwrap(),
            Url::parse("https://minio:1234").unwrap()
        );
        assert_eq!(volumes, vec![]);
        assert_eq!(mounts, vec![]);
    }
}
