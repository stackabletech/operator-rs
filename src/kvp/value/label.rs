use std::{fmt::Display, ops::Deref, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use snafu::{ensure, Snafu};

use crate::kvp::ValueExt;

const LABEL_VALUE_MAX_LEN: usize = 63;

lazy_static! {
    static ref LABEL_VALUE_REGEX: Regex =
        Regex::new(r"^[a-z0-9A-Z]([a-z0-9A-Z-_.]*[a-z0-9A-Z]+)?$").unwrap();
}

#[derive(Debug, PartialEq, Snafu)]
pub enum LabelValueError {
    #[snafu(display(
        "value exceeds the maximum length - expected 63 characters or less, got {length}"
    ))]
    ValueTooLong { length: usize },

    #[snafu(display("value contains non-ascii characters"))]
    ValueNotAscii,

    #[snafu(display("value violates kubernetes format"))]
    ValueInvalid,
}

/// A validated label value of a [`KeyValuePair`](crate::kvp::KeyValuePair).
/// Instances of this struct are always valid. The format and valid characters
/// are described [here][k8s-labels]. It also implements [`Deref`], which
/// enables read-only access to the inner value (a [`String`]). It, however,
/// does not implement [`DerefMut`](std::ops::DerefMut) which would enable
/// unvalidated mutable access to inner values.
///
/// [k8s-labels]: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LabelValue(String);

impl ValueExt for LabelValue {
    type Error = LabelValueError;
}

impl FromStr for LabelValue {
    type Err = LabelValueError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // The length of the value cannot exceed 63 characters, but can be
        // empty
        ensure!(
            input.len() <= LABEL_VALUE_MAX_LEN,
            ValueTooLongSnafu {
                length: input.len()
            }
        );

        // The value cannot contain non-ascii characters
        ensure!(input.is_ascii(), ValueNotAsciiSnafu);

        // The value must use the format specified by Kubernetes
        ensure!(LABEL_VALUE_REGEX.is_match(input), ValueInvalidSnafu);

        Ok(Self(input.to_string()))
    }
}

impl Deref for LabelValue {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LabelValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("a".repeat(64), LabelValueError::ValueTooLong { length: 64 })]
    #[case("foo-", LabelValueError::ValueInvalid)]
    #[case("ä", LabelValueError::ValueNotAscii)]
    fn invalid_value(#[case] input: String, #[case] error: LabelValueError) {
        let err = LabelValue::from_str(&input).unwrap_err();
        assert_eq!(err, error);
    }
}
