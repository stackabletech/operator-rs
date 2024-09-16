use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use url::{ParseError, Url};

use crate::{
    builder::pod::{container::ContainerBuilder, volume::VolumeMountBuilder, PodBuilder},
    commons::{
        authentication::SECRET_BASE_PATH,
        networking::Host,
        secret_class::{SecretClassVolume, SecretClassVolumeError},
        tls_verification::{TlsClientDetails, TlsClientDetailsError},
    },
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display(
        "failed to convert bind credentials (secret class volume) into named Kubernetes volume"
    ))]
    BindCredentials { source: SecretClassVolumeError },

    #[snafu(display("failed to parse LDAP endpoint url"))]
    ParseLdapEndpointUrl { source: ParseError },

    #[snafu(display("failed to add LDAP TLS client details volumes and volume mounts"))]
    AddLdapTlsClientDetailsVolumes { source: TlsClientDetailsError },
}

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProvider {
    /// Host of the LDAP server, for example: `my.ldap.server` or `127.0.0.1`.
    pub hostname: Host,

    /// Port of the LDAP server. If TLS is used defaults to 636 otherwise to 389.
    port: Option<u16>,

    /// LDAP search base, for example: `ou=users,dc=example,dc=org`.
    #[serde(default)]
    pub search_base: String,

    /// LDAP query to filter users, for example: `(memberOf=cn=myTeam,ou=teams,dc=example,dc=org)`.
    #[serde(default)]
    pub search_filter: String,

    /// The name of the LDAP object fields.
    #[serde(default)]
    pub ldap_field_names: FieldNames,

    /// In case you need a special account for searching the LDAP server you can specify it here.
    bind_credentials: Option<SecretClassVolume>,

    /// Use a TLS connection. If not specified no TLS will be used.
    #[serde(flatten)]
    pub tls: TlsClientDetails,
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
        pod_builder.add_volumes(volumes);

        for cb in container_builders {
            cb.add_volume_mounts(mounts.clone());
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

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct FieldNames {
    /// The name of the username field
    #[serde(default = "FieldNames::default_uid")]
    pub uid: String,
    /// The name of the group field
    #[serde(default = "FieldNames::default_group")]
    pub group: String,
    /// The name of the firstname field
    #[serde(default = "FieldNames::default_given_name")]
    pub given_name: String,
    /// The name of the lastname field
    #[serde(default = "FieldNames::default_surname")]
    pub surname: String,
    /// The name of the email field
    #[serde(default = "FieldNames::default_email")]
    pub email: String,
}

impl FieldNames {
    fn default_uid() -> String {
        "uid".to_string()
    }

    fn default_group() -> String {
        "memberof".to_string()
    }

    fn default_given_name() -> String {
        "givenName".to_string()
    }

    fn default_surname() -> String {
        "sn".to_string()
    }

    fn default_email() -> String {
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
mod test {
    use super::*;

    #[test]
    fn test_ldap_minimal() {
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
    fn test_ldap_with_bind_credentials() {
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
    fn test_ldap_full() {
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
        assert_eq!(
            tls_volumes,
            vec![SecretClassVolume {
                secret_class: "ldap-ca-cert".to_string(),
                scope: None,
            }
            .to_volume("ldap-ca-cert-ca-cert")
            .unwrap()]
        );
        assert_eq!(
            tls_mounts,
            vec![VolumeMountBuilder::new(
                "ldap-ca-cert-ca-cert",
                "/stackable/secrets/ldap-ca-cert"
            )
            .build()]
        );

        assert!(ldap.has_bind_credentials());
        assert_eq!(
            ldap.bind_credentials_mount_paths(),
            Some((
                "/stackable/secrets/openldap-bind-credentials/user".to_string(),
                "/stackable/secrets/openldap-bind-credentials/password".to_string()
            ))
        );
        let (ldap_volumes, ldap_mounts) = ldap.volumes_and_mounts().unwrap();
        assert_eq!(
            ldap_volumes,
            vec![
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
            ]
        );
        assert_eq!(
            ldap_mounts,
            vec![
                VolumeMountBuilder::new(
                    "openldap-bind-credentials-bind-credentials",
                    "/stackable/secrets/openldap-bind-credentials"
                )
                .build(),
                VolumeMountBuilder::new("ldap-ca-cert-ca-cert", "/stackable/secrets/ldap-ca-cert")
                    .build()
            ]
        );
    }
}
