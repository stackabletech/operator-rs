use std::{cmp::Ordering, fmt::Display, num::ParseIntError, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;
use snafu::{OptionExt, ResultExt, Snafu};

#[cfg(feature = "darling")]
use darling::FromMeta;

use crate::{Level, ParseLevelError};

lazy_static! {
    static ref VERSION_REGEX: Regex =
        Regex::new(r"^v(?P<major>\d+)(?P<level>[a-z0-9][a-z0-9-]{0,60}[a-z0-9])?$").unwrap();
}

#[derive(Debug, PartialEq, Snafu)]
pub enum VersionParseError {
    #[snafu(display("invalid version format. Input is empty, contains non-ASCII characters or contains more than 63 characters"))]
    InvalidFormat,

    #[snafu(display("failed to parse major version"))]
    ParseMajorVersion { source: ParseIntError },

    #[snafu(display("failed to parse version level"))]
    ParseLevel { source: ParseLevelError },
}

/// A Kubernetes resource version with the `v<MAJOR>(beta/alpha<LEVEL>)`
/// format, for example `v1`, `v2beta1` or `v1alpha2`.
///
/// The version must follow the DNS label format defined [here][1].
///
/// ### See
///
/// - <https://github.com/kubernetes/community/blob/master/contributors/devel/sig-architecture/api-conventions.md#api-conventions>
/// - <https://kubernetes.io/docs/reference/using-api/#api-versioning>
///
/// [1]: https://github.com/kubernetes/design-proposals-archive/blob/main/architecture/identifiers.md#definitions
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub level: Option<Level>,
}

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let captures = VERSION_REGEX.captures(input).context(InvalidFormatSnafu)?;

        let major = captures
            .name("major")
            .expect("internal error: check that the correct match label is specified")
            .as_str()
            .parse::<u64>()
            .context(ParseMajorVersionSnafu)?;

        if let Some(level) = captures.name("level") {
            let level = Level::from_str(level.as_str()).context(ParseLevelSnafu)?;

            Ok(Self {
                level: Some(level),
                major,
            })
        } else {
            Ok(Self { major, level: None })
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }

        match (&self.level, &other.level) {
            (Some(lhs), Some(rhs)) => lhs.cmp(rhs),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.level {
            Some(minor) => write!(f, "v{}{}", self.major, minor),
            None => write!(f, "v{}", self.major),
        }
    }
}

#[cfg(feature = "darling")]
impl FromMeta for Version {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}

impl Version {
    pub fn new(major: u64, minor: Option<Level>) -> Self {
        Self {
            major,
            level: minor,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;

    #[cfg(feature = "darling")]
    use quote::quote;

    #[cfg(feature = "darling")]
    fn parse_meta(tokens: proc_macro2::TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = syn::parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[rstest]
    #[case("v1alpha12", Version { major: 1, level: Some(Level::Alpha(12)) })]
    #[case("v1alpha1", Version { major: 1, level: Some(Level::Alpha(1)) })]
    #[case("v1beta1", Version { major: 1, level: Some(Level::Beta(1)) })]
    #[case("v1", Version { major: 1, level: None })]
    fn valid_version(#[case] input: &str, #[case] expected: Version) {
        let version = Version::from_str(input).expect("valid Kubernetes version");
        assert_eq!(version, expected);
    }

    #[rstest]
    #[case("v1gamma12", VersionParseError::ParseLevel { source: ParseLevelError::UnknownIdentifier })]
    #[case("v1betä1", VersionParseError::InvalidFormat)]
    #[case("1beta1", VersionParseError::InvalidFormat)]
    #[case("", VersionParseError::InvalidFormat)]
    fn invalid_version(#[case] input: &str, #[case] error: VersionParseError) {
        let err = Version::from_str(input).expect_err("invalid Kubernetes version");
        assert_eq!(err, error)
    }

    #[cfg(feature = "darling")]
    #[rstest]
    #[case(quote!(ignore = "v1alpha12"), Version { major: 1, level: Some(Level::Alpha(12)) })]
    #[case(quote!(ignore = "v1alpha1"), Version { major: 1, level: Some(Level::Alpha(1)) })]
    #[case(quote!(ignore = "v1beta1"), Version { major: 1, level: Some(Level::Beta(1)) })]
    #[case(quote!(ignore = "v1"), Version { major: 1, level: None })]
    fn from_meta(#[case] input: proc_macro2::TokenStream, #[case] expected: Version) {
        let meta = parse_meta(input).expect("valid attribute tokens");
        let version = Version::from_meta(&meta).expect("version must parse from attribute");
        assert_eq!(version, expected);
    }
}
