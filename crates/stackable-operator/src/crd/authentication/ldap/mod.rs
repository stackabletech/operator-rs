use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

use crate::commons::{
    networking::HostName, secret_class::SecretClassVolume, tls_verification::TlsClientDetails,
};

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    #[derive(
        Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
    )]
    #[serde(rename_all = "camelCase")]
    pub struct AuthenticationProvider {
        /// Host of the LDAP server, for example: `my.ldap.server` or `127.0.0.1`.
        pub hostname: HostName,

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
}
