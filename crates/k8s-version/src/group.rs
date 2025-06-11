use std::{fmt, ops::Deref, str::FromStr, sync::LazyLock};

use regex::Regex;
use snafu::{Snafu, ensure};

const MAX_GROUP_LENGTH: usize = 253;

static API_GROUP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?:[a-z0-9][a-z0-9-]{0,61}[a-z0-9])\.?)+$")
        .expect("failed to compile API group regex")
});

/// Error variants which can be encountered when creating a new [`Group`] from
/// unparsed input.
#[derive(Debug, PartialEq, Snafu)]
pub enum ParseGroupError {
    #[snafu(display("group must not be empty"))]
    Empty,

    #[snafu(display("group must not be longer than 253 characters"))]
    TooLong,

    #[snafu(display("group must be a valid DNS subdomain"))]
    InvalidFormat,
}

/// A validated Kubernetes group.
///
/// The group string must follow these rules:
///
/// - must be non-empty
/// - must only contain lower case characters
/// - and must be a valid DNS subdomain
///
/// ### See
///
/// - <https://github.com/kubernetes/community/blob/master/contributors/devel/sig-architecture/api-conventions.md#api-conventions>
#[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd)]
pub struct Group(String);

impl FromStr for Group {
    type Err = ParseGroupError;

    fn from_str(group: &str) -> Result<Self, Self::Err> {
        ensure!(!group.is_empty(), EmptySnafu);
        ensure!(group.len() <= MAX_GROUP_LENGTH, TooLongSnafu);
        ensure!(API_GROUP_REGEX.is_match(group), InvalidFormatSnafu);

        Ok(Self(group.to_owned()))
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl Deref for Group {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "serde")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize() {
        let _: Group = serde_yaml::from_str("extensions.k8s.io").expect("group is valid");
    }

    #[test]
    fn serialize() {
        let group = Group("extensions.k8s.io".into());
        assert_eq!(
            "extensions.k8s.io\n",
            serde_yaml::to_string(&group).expect("group must serialize")
        );
    }
}
