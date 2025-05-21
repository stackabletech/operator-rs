use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use snafu::{ResultExt as _, Snafu};
use url::Url;

use crate::{
    builder::{
        self,
        pod::{PodBuilder, container::ContainerBuilder, volume::VolumeMountBuilder},
    },
    commons::{secret_class::SecretClassVolumeError, tls_verification::TlsClientDetailsError},
    constants::secret::SECRET_BASE_PATH,
    crd::authentication::ldap::v1alpha1::{AuthenticationProvider, FieldNames},
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display(
        "failed to convert bind credentials (secret class volume) into named Kubernetes volume"
    ))]
    BindCredentials { source: SecretClassVolumeError },

    #[snafu(display("failed to parse LDAP endpoint url"))]
    ParseLdapEndpointUrl { source: url::ParseError },

    #[snafu(display("failed to add LDAP TLS client details volumes and volume mounts"))]
    AddLdapTlsClientDetailsVolumes { source: TlsClientDetailsError },

    #[snafu(display("failed to add required volumes"))]
    AddVolumes { source: builder::pod::Error },

    #[snafu(display("failed to add required volumeMounts"))]
    AddVolumeMounts {
        source: builder::pod::container::Error,
    },
}

impl AuthenticationProvider {
    /// Returns the LDAP endpoint [`Url`].
    pub fn endpoint_url(&self) -> Result<Url> {
        let url = Url::parse(&format!(
            "{protocol}{server_hostname}:{server_port}",
            protocol = match self.tls.tls {
                None => "ldap://",
                Some(_) => "ldaps://",
            },
            server_hostname = self.hostname,
            server_port = self.port()
        ))
        .context(ParseLdapEndpointUrlSnafu)?;

        Ok(url)
    }

    /// Returns the port to be used, which is either user configured or defaulted based upon TLS usage
    pub fn port(&self) -> u16 {
        self.port
            .unwrap_or(if self.tls.uses_tls() { 636 } else { 389 })
    }

    /// This functions adds
    ///
    /// * The needed volumes to the PodBuilder
    /// * The needed volume_mounts to all the ContainerBuilder in the list (e.g. init + main container)
    ///
    /// This function will handle
    ///
    /// * Bind credentials needed to connect to LDAP server
    /// * Tls secret class used to verify the cert of the LDAP server
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) -> Result<()> {
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
    pub fn volumes_and_mounts(&self) -> Result<(Vec<Volume>, Vec<VolumeMount>)> {
        let mut volumes = Vec::new();
        let mut mounts = Vec::new();

        if let Some(bind_credentials) = &self.bind_credentials {
            let secret_class = &bind_credentials.secret_class;
            let volume_name = format!("{secret_class}-bind-credentials");
            let volume = bind_credentials
                .to_volume(&volume_name)
                .context(BindCredentialsSnafu)?;

            volumes.push(volume);
            mounts.push(
                VolumeMountBuilder::new(volume_name, format!("{SECRET_BASE_PATH}/{secret_class}"))
                    .build(),
            );
        }

        // Add needed TLS volumes
        let (tls_volumes, tls_mounts) = self
            .tls
            .volumes_and_mounts()
            .context(AddLdapTlsClientDetailsVolumesSnafu)?;
        volumes.extend(tls_volumes);
        mounts.extend(tls_mounts);

        Ok((volumes, mounts))
    }

    /// Returns the path of the files containing bind user and password.
    /// This will be None if there are no credentials for this LDAP connection.
    pub fn bind_credentials_mount_paths(&self) -> Option<(String, String)> {
        self.bind_credentials.as_ref().map(|bind_credentials| {
            let secret_class = &bind_credentials.secret_class;
            (
                format!("{SECRET_BASE_PATH}/{secret_class}/user"),
                format!("{SECRET_BASE_PATH}/{secret_class}/password"),
            )
        })
    }

    pub fn has_bind_credentials(&self) -> bool {
        self.bind_credentials.is_some()
    }
}

impl FieldNames {
    pub(super) fn default_uid() -> String {
        "uid".to_string()
    }

    pub(super) fn default_group() -> String {
        "memberof".to_string()
    }

    pub(super) fn default_given_name() -> String {
        "givenName".to_string()
    }

    pub(super) fn default_surname() -> String {
        "sn".to_string()
    }

    pub(super) fn default_email() -> String {
        "mail".to_string()
    }
}

impl Default for FieldNames {
    fn default() -> Self {
        FieldNames {
            uid: Self::default_uid(),
            group: Self::default_group(),
            given_name: Self::default_given_name(),
            surname: Self::default_surname(),
            email: Self::default_email(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commons::secret_class::SecretClassVolume;

    #[test]
    fn minimal() {
        let ldap = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: my.ldap.server
            ",
        )
        .unwrap();

        assert_eq!(ldap.port(), 389);
        assert!(!ldap.tls.uses_tls());
        assert_eq!(ldap.tls.tls_ca_cert_secret_class(), None);
    }

    #[test]
    fn with_bind_credentials() {
        let _ldap = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: my.ldap.server
            port: 389
            searchBase: ou=users,dc=example,dc=org
            bindCredentials:
              secretClass: openldap-bind-credentials
            ",
        )
        .unwrap();
    }

    #[test]
    fn full() {
        let input = r#"
            hostname: my.ldap.server
            port: 42
            searchBase: ou=users,dc=example,dc=org
            bindCredentials:
              secretClass: openldap-bind-credentials
            tls:
              verification:
                server:
                  caCert:
                    secretClass: ldap-ca-cert
        "#;
        let deserializer = serde_yaml::Deserializer::from_str(input);
        let ldap: AuthenticationProvider =
            serde_yaml::with::singleton_map_recursive::deserialize(deserializer).unwrap();

        assert_eq!(ldap.port(), 42);
        assert!(ldap.tls.uses_tls());
        assert_eq!(
            ldap.tls.tls_ca_cert_secret_class(),
            Some("ldap-ca-cert".to_string())
        );
        assert_eq!(
            ldap.tls.tls_ca_cert_mount_path(),
            Some("/stackable/secrets/ldap-ca-cert/ca.crt".to_string())
        );

        let (tls_volumes, tls_mounts) = ldap.tls.volumes_and_mounts().unwrap();
        assert_eq!(tls_volumes, vec![
            SecretClassVolume {
                secret_class: "ldap-ca-cert".to_string(),
                scope: None,
            }
            .to_volume("ldap-ca-cert-ca-cert")
            .unwrap()
        ]);
        assert_eq!(tls_mounts, vec![
            VolumeMountBuilder::new("ldap-ca-cert-ca-cert", "/stackable/secrets/ldap-ca-cert")
                .build()
        ]);

        assert!(ldap.has_bind_credentials());
        assert_eq!(
            ldap.bind_credentials_mount_paths(),
            Some((
                "/stackable/secrets/openldap-bind-credentials/user".to_string(),
                "/stackable/secrets/openldap-bind-credentials/password".to_string()
            ))
        );

        let (ldap_volumes, ldap_mounts) = ldap.volumes_and_mounts().unwrap();
        assert_eq!(ldap_volumes, vec![
            SecretClassVolume {
                secret_class: "openldap-bind-credentials".to_string(),
                scope: None,
            }
            .to_volume("openldap-bind-credentials-bind-credentials")
            .unwrap(),
            SecretClassVolume {
                secret_class: "ldap-ca-cert".to_string(),
                scope: None,
            }
            .to_volume("ldap-ca-cert-ca-cert")
            .unwrap()
        ]);
        assert_eq!(ldap_mounts, vec![
            VolumeMountBuilder::new(
                "openldap-bind-credentials-bind-credentials",
                "/stackable/secrets/openldap-bind-credentials"
            )
            .build(),
            VolumeMountBuilder::new("ldap-ca-cert-ca-cert", "/stackable/secrets/ldap-ca-cert")
                .build()
        ]);
    }
}
