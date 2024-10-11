use std::{
    fmt::{Debug, Display},
    ops::Deref,
    str::FromStr,
    sync::LazyLock,
};

use regex::Regex;
use snafu::{ensure, Snafu};

use crate::kvp::Value;

const LABEL_VALUE_MAX_LEN: usize = 63;

// Lazily initialized regular expressions
static LABEL_VALUE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9A-Z]([a-z0-9A-Z-_.]*[a-z0-9A-Z]+)?$")
        .expect("failed to compile value regex")
});

/// The error type for label value parse/validation operations.
#[derive(Debug, PartialEq, Snafu)]
pub enum LabelValueError {
    /// Indicates that the label value exceeds the maximum length of 63 ASCII
    /// characters. It additionally reports how many characters were
    /// encountered during parsing / validation.
    #[snafu(display(
        "value exceeds the maximum length - expected 63 characters or less, got {length}"
    ))]
    ValueTooLong { length: usize },

    /// Indicates that the label value contains non-ASCII characters which the
    /// Kubernetes spec does not permit.
    #[snafu(display("value contains non-ascii characters"))]
    ValueNotAscii,

    /// Indicates that the label value violates the specified Kubernetes format.
    #[snafu(display("value violates kubernetes format"))]
    ValueInvalid,
}

/// A validated Kubernetes label value.
///
/// Instances of this struct are always valid. The format and valid characters
/// are described [here][k8s-labels]. It also implements [`Deref`], which
/// enables read-only access to the inner value (a [`String`]). It, however,
/// does not implement [`DerefMut`](std::ops::DerefMut) which would enable
/// unvalidated mutable access to inner values.
///
/// [k8s-labels]: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct LabelValue(String);

impl Debug for LabelValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Value for LabelValue {
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
    type Target = str;

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
