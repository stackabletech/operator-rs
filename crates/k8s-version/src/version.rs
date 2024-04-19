use std::{cmp::Ordering, fmt::Display, num::ParseIntError, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;
use snafu::{OptionExt, ResultExt, Snafu};

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
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
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

        let level = captures
            .name("level")
            .expect("internal error: check that the correct match label is specified")
            .as_str();

        if level.is_empty() {
            return Ok(Self { major, level: None });
        }

        let level = Level::from_str(level).context(ParseLevelSnafu)?;

        Ok(Self {
            level: Some(level),
            major,
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.major.partial_cmp(&other.major) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }

        match (&self.level, &other.level) {
            (Some(lhs), Some(rhs)) => lhs.partial_cmp(rhs),
            (Some(_), None) => Some(Ordering::Less),
            (None, Some(_)) => Some(Ordering::Greater),
            (None, None) => Some(Ordering::Equal),
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

    #[rstest]
    #[case("v1alpha12")]
    #[case("v1alpha1")]
    #[case("v1beta1")]
    #[case("v1")]
    fn valid_version(#[case] input: &str) {
        let version = Version::from_str(input).unwrap();
        assert_eq!(version.to_string(), input);
    }

    // #[rstest]
    // #[case("v1gamma12", VersionParseError::ParseLevel { source: ParseLevelError::InvalidLevel })]
    // #[case("v1betä1", VersionParseError::InvalidFormat)]
    // #[case("1beta1", VersionParseError::InvalidStart)]
    // #[case("", VersionParseError::InvalidFormat)]
    // #[case("v0", VersionParseError::LeadingZero)]
    // fn invalid_version(#[case] input: &str, #[case] error: VersionParseError) {
    //     let err = Version::from_str(input).unwrap_err();
    //     assert_eq!(err, error)
    // }
}
