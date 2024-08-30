use std::{fmt::Display, ops::Deref};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validation;

/// A validated hostname type, for use in CRDs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct Hostname(#[validate(regex(path = "validation::RFC_1123_SUBDOMAIN_REGEX"))] String);

impl TryFrom<String> for Hostname {
    type Error = validation::Errors;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validation::is_rfc_1123_subdomain(&value)?;
        Ok(Hostname(value))
    }
}

impl From<Hostname> for String {
    fn from(value: Hostname) -> Self {
        value.0
    }
}

impl Display for Hostname {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for Hostname {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A validated kerberos realm name type, for use in CRDs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct KerberosRealmName(
    #[validate(regex(path = "validation::KERBEROS_REALM_NAME_REGEX"))] String,
);

impl TryFrom<String> for KerberosRealmName {
    type Error = validation::Errors;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validation::is_kerberos_realm_name(&value)?;
        Ok(KerberosRealmName(value))
    }
}
impl From<KerberosRealmName> for String {
    fn from(value: KerberosRealmName) -> Self {
        value.0
    }
}
impl Display for KerberosRealmName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl Deref for KerberosRealmName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
