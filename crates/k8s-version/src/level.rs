use std::{
    cmp::Ordering,
    fmt::Display,
    num::ParseIntError,
    ops::{Add, AddAssign, Sub, SubAssign},
    str::FromStr,
};

use lazy_static::lazy_static;
use regex::Regex;
use snafu::{OptionExt, ResultExt, Snafu};

#[cfg(feature = "darling")]
use darling::FromMeta;

lazy_static! {
    static ref LEVEL_REGEX: Regex =
        Regex::new(r"^(?P<identifier>[a-z]+)(?P<version>\d+)$").unwrap();
}

/// Error variants which can be encountered when creating a new [`Level`] from
/// unparsed input.
#[derive(Debug, PartialEq, Snafu)]
pub enum ParseLevelError {
    #[snafu(display("invalid level format, expected beta<VERSION>/alpha<VERSION>"))]
    InvalidFormat,

    #[snafu(display("failed to parse level version"))]
    ParseVersion { source: ParseIntError },

    #[snafu(display("unknown level identifier"))]
    UnknownIdentifier,
}

/// A minor Kubernetes resource version with the `beta/alpha<VERSION>` format.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Level {
    /// Beta-level minor version, `beta<VERSION>`.
    Beta(u64),

    /// Alpha-level minor version, `alpha<VERSION>`.
    Alpha(u64),
}

impl FromStr for Level {
    type Err = ParseLevelError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let captures = LEVEL_REGEX.captures(input).context(InvalidFormatSnafu)?;

        let identifier = captures
            .name("identifier")
            .expect("internal error: check that the correct match label is specified")
            .as_str();

        let version = captures
            .name("version")
            .expect("internal error: check that the correct match label is specified")
            .as_str()
            .parse::<u64>()
            .context(ParseVersionSnafu)?;

        match identifier {
            "alpha" => Ok(Self::Alpha(version)),
            "beta" => Ok(Self::Beta(version)),
            _ => UnknownIdentifierSnafu.fail(),
        }
    }
}

impl PartialOrd for Level {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Level {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            Level::Beta(sb) => match other {
                Level::Beta(ob) => sb.cmp(ob),
                Level::Alpha(_) => Ordering::Greater,
            },
            Level::Alpha(sa) => match other {
                Level::Beta(_) => Ordering::Less,
                Level::Alpha(oa) => sa.cmp(oa),
            },
        }
    }
}

impl<T> Add<T> for Level
where
    T: Into<u64>,
{
    type Output = Level;

    fn add(self, rhs: T) -> Self::Output {
        match self {
            Level::Beta(b) => Level::Beta(b + rhs.into()),
            Level::Alpha(a) => Level::Alpha(a + rhs.into()),
        }
    }
}

impl<T> AddAssign<T> for Level
where
    T: Into<u64>,
{
    fn add_assign(&mut self, rhs: T) {
        match self {
            Level::Beta(b) => *b + rhs.into(),
            Level::Alpha(a) => *a + rhs.into(),
        };
    }
}

impl<T> Sub<T> for Level
where
    T: Into<u64>,
{
    type Output = Level;

    fn sub(self, rhs: T) -> Self::Output {
        match self {
            Level::Beta(b) => Level::Beta(b - rhs.into()),
            Level::Alpha(a) => Level::Alpha(a - rhs.into()),
        }
    }
}

impl<T> SubAssign<T> for Level
where
    T: Into<u64>,
{
    fn sub_assign(&mut self, rhs: T) {
        match self {
            Level::Beta(b) => *b - rhs.into(),
            Level::Alpha(a) => *a - rhs.into(),
        };
    }
}

impl Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Beta(beta) => write!(f, "beta{}", beta),
            Level::Alpha(alpha) => write!(f, "alpha{}", alpha),
        }
    }
}

#[cfg(feature = "darling")]
impl FromMeta for Level {
    fn from_string(value: &str) -> darling::Result<Self> {
        Self::from_str(value).map_err(darling::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;
    use rstest_reuse::*;

    use super::*;

    #[cfg(feature = "darling")]
    use quote::quote;

    #[cfg(feature = "darling")]
    fn parse_meta(tokens: proc_macro2::TokenStream) -> ::std::result::Result<syn::Meta, String> {
        let attribute: syn::Attribute = syn::parse_quote!(#[#tokens]);
        Ok(attribute.meta)
    }

    #[template]
    #[rstest]
    #[case(Level::Beta(1), Level::Alpha(1), Ordering::Greater)]
    #[case(Level::Alpha(1), Level::Beta(1), Ordering::Less)]
    #[case(Level::Alpha(2), Level::Alpha(1), Ordering::Greater)]
    #[case(Level::Alpha(2), Level::Alpha(2), Ordering::Equal)]
    #[case(Level::Alpha(1), Level::Alpha(2), Ordering::Less)]
    #[case(Level::Beta(2), Level::Beta(1), Ordering::Greater)]
    #[case(Level::Beta(2), Level::Beta(2), Ordering::Equal)]
    #[case(Level::Beta(1), Level::Beta(2), Ordering::Less)]
    fn ord_cases(#[case] input: Level, #[case] other: Level, #[case] expected: Ordering) {}

    #[apply(ord_cases)]
    fn ord(input: Level, other: Level, expected: Ordering) {
        assert_eq!(input.cmp(&other), expected)
    }

    #[apply(ord_cases)]
    fn partial_ord(input: Level, other: Level, expected: Ordering) {
        assert_eq!(input.partial_cmp(&other), Some(expected))
    }

    #[cfg(feature = "darling")]
    #[rstest]
    #[case(quote!(ignore = "alpha12"), Level::Alpha(12))]
    #[case(quote!(ignore = "alpha1"), Level::Alpha(1))]
    #[case(quote!(ignore = "beta1"), Level::Beta(1))]
    fn from_meta(#[case] input: proc_macro2::TokenStream, #[case] expected: Level) {
        let meta = parse_meta(input).expect("valid attribute tokens");
        let version = Level::from_meta(&meta).expect("level must parse from attribute");
        assert_eq!(version, expected);
    }
}
