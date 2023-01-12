use crate::builder::ContainerBuilder;
use crate::commons::tls::Tls;
use crate::{builder::PodBuilder, commons::secret_class::SecretClassVolume};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::tls::{CaCert, TlsServerVerification, TlsVerification};

const SECRET_BASE_PATH: &str = "/stackable/secrets";

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LdapAuthenticationProvider {
    /// Hostname of the LDAP server
    pub hostname: String,
    /// Port of the LDAP server. If TLS is used defaults to 636 otherwise to 389
    pub port: Option<u16>,
    /// LDAP search base
    #[serde(default)]
    pub search_base: String,
    /// LDAP query to filter users
    #[serde(default)]
    pub search_filter: String,
    /// The name of the LDAP object fields
    #[serde(default)]
    pub ldap_field_names: LdapFieldNames,
    /// In case you need a special account for searching the LDAP server you can specify it here
    pub bind_credentials: Option<SecretClassVolume>,
    /// Use a TLS connection. If not specified no TLS will be used
    pub tls: Option<Tls>,
}

impl LdapAuthenticationProvider {
    pub fn default_port(&self) -> u16 {
        match self.tls {
            None => 389,
            Some(_) => 636,
        }
    }

    /// This functions adds
    ///
    /// * The needed volumes to the Pod
    /// * The needed volume_mounts to all the Containers in the list (e.g. init + main container)
    ///
    /// This function will handle
    ///
    /// * Bind credentials needed to connect to LDAP server
    /// * Tls secret class used to verify the cert of the LDAP server
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: &mut [ContainerBuilder],
    ) {
        if let Some(bind_credentials) = &self.bind_credentials {
            let secret_class = &bind_credentials.secret_class;
            let volume_name = format!("{secret_class}-bind-credentials");

            pod_builder.add_volume(bind_credentials.to_volume(&volume_name));
            for cb in container_builders.iter_mut() {
                cb.add_volume_mount(&volume_name, format!("{SECRET_BASE_PATH}{secret_class}"));
            }
        }
        if let Some(Tls {
            verification:
                TlsVerification::Server(TlsServerVerification {
                    ca_cert: CaCert::SecretClass(secret_class),
                }),
        }) = &self.tls
        {
            let volume_name = format!("{secret_class}-ca-cert");
            let volume = SecretClassVolume {
                secret_class: secret_class.to_string(),
                scope: None,
            }
            .to_volume(&volume_name);

            pod_builder.add_volume(volume);
            for cb in container_builders.iter_mut() {
                cb.add_volume_mount(&volume_name, format!("{SECRET_BASE_PATH}{secret_class}"));
            }
        }
    }

    /// Returns the path of the files containing bind user and password
    pub fn bind_credentials_mount_paths(&self) -> Option<(String, String)> {
        self.bind_credentials.as_ref().map(|bind_credentials| {
            let secret_class = &bind_credentials.secret_class;
            (
                format!("{SECRET_BASE_PATH}{secret_class}/user"),
                format!("{SECRET_BASE_PATH}{secret_class}/password"),
            )
        })
    }

    /// Returns the path of the ca.crt that should be used to verify the LDAP server certificate
    pub fn tls_ca_cert_mount_path(&self) -> Option<String> {
        if let Some(Tls {
            verification:
                TlsVerification::Server(TlsServerVerification {
                    ca_cert: CaCert::SecretClass(secret_class),
                }),
        }) = &self.tls
        {
            Some(format!("{SECRET_BASE_PATH}{secret_class}/ca.crt"))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LdapFieldNames {
    /// The name of the username field
    #[serde(default = "LdapFieldNames::default_uid")]
    pub uid: String,
    /// The name of the group field
    #[serde(default = "LdapFieldNames::default_group")]
    pub group: String,
    /// The name of the firstname field
    #[serde(default = "LdapFieldNames::default_given_name")]
    pub given_name: String,
    /// The name of the lastname field
    #[serde(default = "LdapFieldNames::default_surname")]
    pub surname: String,
    /// The name of the email field
    #[serde(default = "LdapFieldNames::default_email")]
    pub email: String,
}

impl LdapFieldNames {
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

impl Default for LdapFieldNames {
    fn default() -> Self {
        LdapFieldNames {
            uid: Self::default_uid(),
            group: Self::default_group(),
            given_name: Self::default_given_name(),
            surname: Self::default_surname(),
            email: Self::default_email(),
        }
    }
}
