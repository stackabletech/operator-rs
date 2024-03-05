use const_format::concatcp;

/// The root CA common name DN `CN=Stackable Root CA`.
pub const ROOT_CA_COMMON_NAME_DN: &str = "CN=Stackable Root CA";

/// A common organization DN `O=Stackable GmbH`.
pub const ORGANIZATION_DN: &str = "O=Stackable GmbH";

/// The default CA validity time span of one hour (3600 seconds).
pub const DEFAULT_CA_VALIDITY_SECONDS: u64 = 3600;

/// A common country DN `C=DE`.
pub const COUNTRY_DN: &str = "C=DE";

/// The root CA subject name containing the common name, organization name and
/// country.
pub const ROOT_CA_SUBJECT: &str = concatcp!(
    ROOT_CA_COMMON_NAME_DN,
    ",",
    ORGANIZATION_DN,
    ",",
    COUNTRY_DN
);
