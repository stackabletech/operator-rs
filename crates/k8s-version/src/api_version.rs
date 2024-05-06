use std::{cmp::Ordering, fmt::Display, str::FromStr};

use snafu::{ResultExt, Snafu};

#[cfg(feature = "darling")]
use darling::FromMeta;

use crate::{Group, ParseGroupError, ParseVersionError, Version};

/// Error variants which can be encountered when creating a new [`ApiVersion`]
/// from unparsed input.
#[derive(Debug, PartialEq, Snafu)]
pub enum ParseApiVersionError {
    #[snafu(display("failed to parse version"))]
    ParseVersion { source: ParseVersionError },

    #[snafu(display("failed to parse group"))]
    ParseGroup { source: ParseGroupError },
}

/// A Kubernetes API version, following the `(<GROUP>/)<VERSION>` format.
///
/// The `<VERSION>` string must follow the DNS label format defined in the [Kubernetes design proposals archive][1].
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
    pub group: Option<Group>,
    pub version: Version,
}

impl FromStr for ApiVersion {
    type Err = ParseApiVersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (group, version) = if let Some((group, version)) = input.split_once('/') {
            let group = Group::from_str(group).context(ParseGroupSnafu)?;

            (
                Some(group),
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
            Some(group) => write!(f, "{group}/{version}", version = self.version),
            None => write!(f, "{version}", version = self.version),
        }
    }
}

#[cfg(feature = "darling")]
impl FromMeta for ApiVersion {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}

impl ApiVersion {
    /// Create a new Kubernetes API version.
    pub fn new(group: Option<Group>, version: Version) -> Self {
        Self { group, version }
    }

    /// Try to create a new Kubernetes API version based on the unvalidated
    /// `group` string.
    pub fn try_new(group: Option<&str>, version: Version) -> Result<Self, ParseApiVersionError> {
        let group = group
            .map(|g| g.parse())
            .transpose()
            .context(ParseGroupSnafu)?;

        Ok(Self { group, version })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Level;

    use rstest::rstest;

    #[cfg(feature = "darling")]
    use quote::quote;

    #[cfg(feature = "darling")]
    fn parse_meta(tokens: proc_macro2::TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = syn::parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[rstest]
    #[case("extensions/v1beta1", ApiVersion { group: Some("extensions".parse().unwrap()), version: Version { major: 1, level: Some(Level::Beta(1)) } })]
    #[case("v1beta1", ApiVersion { group: None, version: Version { major: 1, level: Some(Level::Beta(1)) } })]
    #[case("v1", ApiVersion { group: None, version: Version { major: 1, level: None } })]
    fn valid_api_version(#[case] input: &str, #[case] expected: ApiVersion) {
        let api_version = ApiVersion::from_str(input).expect("valid Kubernetes api version");
        assert_eq!(api_version, expected);
    }

    #[rstest]
    #[case("extensions/beta1", ParseApiVersionError::ParseVersion { source: ParseVersionError::InvalidFormat })]
    #[case("/v1beta1", ParseApiVersionError::ParseGroup { source: ParseGroupError::Empty })]
    fn invalid_api_version(#[case] input: &str, #[case] error: ParseApiVersionError) {
        let err = ApiVersion::from_str(input).expect_err("invalid Kubernetes api versions");
        assert_eq!(err, error);
    }

    #[rstest]
    #[case(Version {major: 1, level: Some(Level::Alpha(2))}, Version {major: 1, level: Some(Level::Alpha(1))}, Ordering::Greater)]
    #[case(Version {major: 1, level: Some(Level::Alpha(1))}, Version {major: 1, level: Some(Level::Alpha(1))}, Ordering::Equal)]
    #[case(Version {major: 1, level: Some(Level::Alpha(1))}, Version {major: 1, level: Some(Level::Alpha(2))}, Ordering::Less)]
    #[case(Version {major: 1, level: None}, Version {major: 1, level: Some(Level::Alpha(2))}, Ordering::Greater)]
    #[case(Version {major: 1, level: None}, Version {major: 1, level: Some(Level::Beta(2))}, Ordering::Greater)]
    #[case(Version {major: 1, level: None}, Version {major: 1, level: None}, Ordering::Equal)]
    #[case(Version {major: 1, level: None}, Version {major: 2, level: None}, Ordering::Less)]
    fn partial_ord(#[case] input: Version, #[case] other: Version, #[case] expected: Ordering) {
        assert_eq!(input.partial_cmp(&other), Some(expected));
    }

    #[cfg(feature = "darling")]
    #[rstest]
    #[case(quote!(ignore = "extensions/v1beta1"), ApiVersion { group: Some("extensions".parse().unwrap()), version: Version { major: 1, level: Some(Level::Beta(1)) } })]
    #[case(quote!(ignore = "v1beta1"), ApiVersion { group: None, version: Version { major: 1, level: Some(Level::Beta(1)) } })]
    #[case(quote!(ignore = "v1"), ApiVersion { group: None, version: Version { major: 1, level: None } })]
    fn from_meta(#[case] input: proc_macro2::TokenStream, #[case] expected: ApiVersion) {
        let meta = parse_meta(input).expect("valid attribute tokens");
        let api_version = ApiVersion::from_meta(&meta).expect("version must parse from attribute");
        assert_eq!(api_version, expected);
    }
}
