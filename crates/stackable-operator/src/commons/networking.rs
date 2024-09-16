use std::{fmt::Display, net::IpAddr, ops::Deref, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validation;

/// A validated hostname type conforming to RFC 1123, e.g. not an IPv6 address.
#[derive(
    Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct Hostname(#[validate(regex(path = "validation::RFC_1123_SUBDOMAIN_REGEX"))] String);

impl FromStr for Hostname {
    type Err = validation::Errors;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validation::is_rfc_1123_subdomain(value)?;
        Ok(Hostname(value.to_owned()))
    }
}

impl TryFrom<String> for Hostname {
    type Error = validation::Errors;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
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

/// A validated host (either a [`Hostname`] or IP address) type.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[serde(try_from = "String", into = "String")]
pub enum Host {
    IpAddress(IpAddr),
    Hostname(Hostname),
}

impl JsonSchema for Host {
    fn schema_name() -> String {
        "Host".to_owned()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }
}

impl FromStr for Host {
    type Err = validation::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Ok(ip) = value.parse::<IpAddr>() {
            return Ok(Host::IpAddress(ip));
        }

        if let Ok(hostname) = value.parse() {
            return Ok(Host::Hostname(hostname));
        };

        Err(validation::Error::InvalidHost {})
    }
}

impl TryFrom<String> for Host {
    type Error = validation::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Host> for String {
    fn from(value: Host) -> Self {
        value.to_string()
    }
}

impl Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Host::IpAddress(ip) => write!(f, "{ip}"),
            Host::Hostname(hostname) => write!(f, "{hostname}"),
        }
    }
}

impl Host {
    /// Formats the host in such a way that it can be used in URLs.
    pub fn as_url_host(&self) -> String {
        match self {
            Host::IpAddress(ip) => match ip {
                IpAddr::V4(ip) => ip.to_string(),
                IpAddr::V6(ip) => format!("[{ip}]"),
            },
            Host::Hostname(hostname) => hostname.to_string(),
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
    // Well this is also a valid hostname I guess
    #[case("1.2.3.4")]
    fn test_host_and_hostname_parsing_success(#[case] hostname: String) {
        let parsed_hostname: Hostname = hostname.parse().expect("hostname can not be parsed");
        // Every hostname is also a valid host
        let parsed_host: Host = hostname.parse().expect("host can not be parsed");

        // Also test the round-trip
        assert_eq!(parsed_hostname.to_string(), hostname);
        assert_eq!(parsed_host.to_string(), hostname);
    }

    #[rstest]
    #[case("")]
    #[case("foo.bar:1234")]
    #[case("fe80::1")]
    fn test_hostname_parsing_invalid_input(#[case] hostname: &str) {
        assert!(hostname.parse::<Hostname>().is_err());
    }

    #[rstest]
    #[case("foo", "foo")]
    #[case("foo.bar", "foo.bar")]
    #[case("1.2.3.4", "1.2.3.4")]
    #[case("fe80::1", "[fe80::1]")]
    fn test_host_parsing_success(#[case] host: &str, #[case] expected_url_host: &str) {
        let parsed_host: Host = host.parse().expect("host can not be parsed");

        // Also test the round-trip
        assert_eq!(parsed_host.to_string(), host);

        assert_eq!(parsed_host.as_url_host(), expected_url_host);
    }
}
