use std::{cmp::Ordering, fmt::Display, str::FromStr};

use snafu::{ResultExt, Snafu};

#[cfg(feature = "darling")]
use darling::FromMeta;

use crate::{Version, VersionParseError};

#[derive(Debug, PartialEq, Snafu)]
pub enum ApiVersionParseError {
    #[snafu(display("failed to parse version"))]
    ParseVersion { source: VersionParseError },
}

/// A Kubernetes API version with the `(<GROUP>/)<VERSION>` format, for example
/// `certificates.k8s.io/v1beta1`, `extensions/v1beta1` or `v1`.
///
/// The `<VERSION>` string must follow the DNS label format defined [here][1].
/// The `<GROUP>` string must be lower case and must be a valid DNS subdomain.
///
/// ### See
///
/// - <https://github.com/kubernetes/community/blob/master/contributors/devel/sig-architecture/api-conventions.md#api-conventions>
/// - <https://kubernetes.io/docs/reference/using-api/#api-versioning>
/// - <https://kubernetes.io/docs/reference/using-api/#api-groups>
///
/// [1]: https://github.com/kubernetes/design-proposals-archive/blob/main/architecture/identifiers.md#definitions
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ApiVersion {
    pub group: Option<String>,
    pub version: Version,
}

impl FromStr for ApiVersion {
    type Err = ApiVersionParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (group, version) = if let Some((group, version)) = input.split_once('/') {
            // TODO (Techassi): Validate group
            (
                Some(group.to_string()),
                Version::from_str(version).context(ParseVersionSnafu)?,
            )
        } else {
            (None, Version::from_str(input).context(ParseVersionSnafu)?)
        };

        Ok(Self { group, version })
    }
}

impl PartialOrd for ApiVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.group.partial_cmp(&other.group) {
            Some(Ordering::Equal) => {}
            _ => return None,
        }
        self.version.partial_cmp(&other.version)
    }
}

impl Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.group {
            Some(group) => write!(f, "{}/{}", group, self.version),
            None => write!(f, "{}", self.version),
        }
    }
}

#[cfg(feature = "darling")]
impl FromMeta for ApiVersion {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}
