use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use snafu::{ResultExt as _, Snafu};
use url::Url;

use crate::{
    builder::pod::{PodBuilder, container::ContainerBuilder, volume::VolumeMountBuilder},
    client::Client,
    commons::{secret_class::SecretClassVolumeError, tls_verification::TlsClientDetailsError},
    constants::secret::SECRET_BASE_PATH,
    crd::s3::{
        connection::ResolvedConnection,
        v1alpha1::{ConnectionSpec, InlineConnectionOrReference, Region, S3Connection},
    },
};

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

impl ConnectionSpec {
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

impl Region {
    /// Having it as `const &str` as well, so we don't always allocate a [`String`] just for comparisons
    pub const DEFAULT_REGION_NAME: &str = "us-east-1";

    pub(super) fn default_region_name() -> String {
        Self::DEFAULT_REGION_NAME.to_string()
    }

    /// Returns if the region sticks to the Stackable defaults.
    ///
    /// Some products don't really support configuring the region.
    /// This function can be used to determine if a warning or error should be raised to inform the
    /// user of this situation.
    pub fn is_default_config(&self) -> bool {
        self.name == Self::DEFAULT_REGION_NAME
    }
}

impl Default for Region {
    fn default() -> Self {
        Self {
            name: Self::default_region_name(),
        }
    }
}

impl InlineConnectionOrReference {
    pub async fn resolve(
        self,
        client: &Client,
        namespace: &str,
    ) -> Result<ResolvedConnection, ConnectionError> {
        match self {
            Self::Inline(inline) => Ok(inline),
            Self::Reference(reference) => {
                let connection_spec = client
                    .get::<S3Connection>(&reference, namespace)
                    .await
                    .context(RetrieveS3ConnectionSnafu {
                        s3_connection: reference,
                    })?
                    .spec;

                Ok(connection_spec)
            }
        }
    }
}
