use const_format::concatcp;

pub const ROOT_CA_COMMON_NAME_DN: &str = "CN=Stackable Root CA";
pub const ORGANIZATION_DN: &str = "O=Stackable GmbH";
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
