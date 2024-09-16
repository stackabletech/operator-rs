use std::{fmt::Display, net::IpAddr, ops::Deref, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::Snafu;

use crate::validation;

/// A validated domain name type conforming to RFC 1123, e.g. an IPv4, but not an IPv6 address.
#[derive(
    Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct DomainName(#[validate(regex(path = "validation::RFC_1123_SUBDOMAIN_REGEX"))] String);

impl FromStr for DomainName {
    type Err = validation::Errors;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validation::is_rfc_1123_subdomain(value)?;
        Ok(DomainName(value.to_owned()))
    }
}

impl TryFrom<String> for DomainName {
    type Error = validation::Errors;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<DomainName> for String {
    fn from(value: DomainName) -> Self {
        value.0
    }
}

impl Display for DomainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for DomainName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Snafu)]
pub enum HostNameParseError {
    #[snafu(display(
        "the given hostname '{hostname}' is not a valid hostname, which needs to be either a domain name or IP address"
    ))]
    InvalidHostname { hostname: String },
}

/// A validated hostname (either a [`DomainName`] or IP address) type.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[serde(try_from = "String", into = "String")]
pub enum HostName {
    IpAddress(IpAddr),
    DomainName(DomainName),
}

impl JsonSchema for HostName {
    fn schema_name() -> String {
        "HostName".to_owned()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }
}

impl FromStr for HostName {
    type Err = HostNameParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Ok(ip) = value.parse::<IpAddr>() {
            return Ok(HostName::IpAddress(ip));
        }

        if let Ok(domain_name) = value.parse() {
            return Ok(HostName::DomainName(domain_name));
        };

        InvalidHostnameSnafu {
            hostname: value.to_owned(),
        }
        .fail()
    }
}

impl TryFrom<String> for HostName {
    type Error = HostNameParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<HostName> for String {
    fn from(value: HostName) -> Self {
        value.to_string()
    }
}

impl Display for HostName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostName::IpAddress(ip) => write!(f, "{ip}"),
            HostName::DomainName(domain_name) => write!(f, "{domain_name}"),
        }
    }
}

impl HostName {
    /// Formats the host in such a way that it can be used in URLs.
    pub fn as_url_host(&self) -> String {
        match self {
            HostName::IpAddress(ip) => match ip {
                IpAddr::V4(ip) => ip.to_string(),
                IpAddr::V6(ip) => format!("[{ip}]"),
            },
            HostName::DomainName(domain_name) => domain_name.to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    #[rstest]
    #[case("foo")]
    #[case("foo.bar")]
    // This is also a valid domain name
    #[case("1.2.3.4")]
    fn test_domain_name_and_host_name_parsing_success(#[case] domain_name: String) {
        let parsed_domain_name: DomainName =
            domain_name.parse().expect("domain name can not be parsed");
        // Every domain name is also a valid host name
        let parsed_host_name: HostName = domain_name.parse().expect("host name can not be parsed");

        // Also test the round-trip
        assert_eq!(parsed_domain_name.to_string(), domain_name);
        assert_eq!(parsed_host_name.to_string(), domain_name);
    }

    #[rstest]
    #[case("")]
    #[case("foo.bar:1234")]
    #[case("fe80::1")]
    fn test_domain_name_parsing_invalid_input(#[case] domain_name: &str) {
        assert!(domain_name.parse::<DomainName>().is_err());
    }

    #[rstest]
    #[case("foo", "foo")]
    #[case("foo.bar", "foo.bar")]
    #[case("1.2.3.4", "1.2.3.4")]
    #[case("fe80::1", "[fe80::1]")]
    fn test_host_name_parsing_success(#[case] host: &str, #[case] expected_url_host: &str) {
        let parsed_host_name: HostName = host.parse().expect("host can not be parsed");

        // Also test the round-trip
        assert_eq!(parsed_host_name.to_string(), host);

        assert_eq!(parsed_host_name.as_url_host(), expected_url_host);
    }
}
