use std::{cmp::Ordering, fmt::Display, num::ParseIntError, str::FromStr, sync::LazyLock};

use regex::Regex;
use snafu::{OptionExt, ResultExt, Snafu};

use crate::{Level, ParseLevelError};

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "darling")]
mod darling;

static VERSION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^v(?P<major>\d+)(?P<level>[a-z0-9][a-z0-9-]{0,60}[a-z0-9])?$")
        .expect("failed to compile version regex")
});

/// Error variants which can be encountered when creating a new [`Version`] from
/// unparsed input.
#[derive(Debug, PartialEq, Snafu)]
pub enum ParseVersionError {
    #[snafu(display(
        "invalid version format. Input is empty, contains non-ASCII characters or contains more than 63 characters"
    ))]
    InvalidFormat,

    #[snafu(display("failed to parse major version"))]
    ParseMajorVersion { source: ParseIntError },

    #[snafu(display("failed to parse version level"))]
    ParseLevel { source: ParseLevelError },
}

/// A Kubernetes resource version, following the
/// `v<MAJOR>(alpha<LEVEL|beta<LEVEL>)` format.
///
/// The version must follow the DNS label format defined in the
/// [Kubernetes design proposals archive][1].
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
    type Err = ParseVersionError;

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
            Some(level) => write!(f, "v{major}{level}", major = self.major),
            None => write!(f, "v{major}", major = self.major),
        }
    }
}

impl Version {
    pub fn new(major: u64, level: Option<Level>) -> Self {
        Self { major, level }
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;
    use rstest_reuse::{apply, template};

    use super::*;

    #[template]
    #[rstest]
    #[case(Version {major: 1, level: Some(Level::Beta(1))}, Version {major: 1, level: Some(Level::Alpha(1))}, Ordering::Greater)]
    #[case(Version {major: 1, level: Some(Level::Alpha(1))}, Version {major: 1, level: Some(Level::Beta(1))}, Ordering::Less)]
    #[case(Version {major: 1, level: Some(Level::Beta(1))}, Version {major: 1, level: Some(Level::Beta(1))}, Ordering::Equal)]
    fn ord_cases(#[case] input: Version, #[case] other: Version, #[case] expected: Ordering) {}

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
    #[case("v1gamma12", ParseVersionError::ParseLevel { source: ParseLevelError::UnknownIdentifier })]
    #[case("v1betä1", ParseVersionError::InvalidFormat)]
    #[case("1beta1", ParseVersionError::InvalidFormat)]
    #[case("", ParseVersionError::InvalidFormat)]
    fn invalid_version(#[case] input: &str, #[case] error: ParseVersionError) {
        let err = Version::from_str(input).expect_err("invalid Kubernetes version");
        assert_eq!(err, error)
    }

    #[apply(ord_cases)]
    fn ord(input: Version, other: Version, expected: Ordering) {
        assert_eq!(input.cmp(&other), expected)
    }

    #[apply(ord_cases)]
    fn partial_ord(input: Version, other: Version, expected: Ordering) {
        assert_eq!(input.partial_cmp(&other), Some(expected))
    }
}
