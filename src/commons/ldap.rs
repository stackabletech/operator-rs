use crate::commons::secret_class::SecretClassVolume;
use crate::commons::tls::Tls;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
