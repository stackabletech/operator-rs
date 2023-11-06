use std::{fmt::Display, ops::Deref, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;
use snafu::{ensure, ResultExt, Snafu};

const LABEL_KEY_PREFIX_MAX_LEN: usize = 253;
const LABEL_KEY_NAME_MAX_LEN: usize = 63;

lazy_static! {
    static ref LABEL_KEY_PREFIX_REGEX: Regex =
        Regex::new(r"^[a-zA-Z](\.?[a-zA-Z0-9-])*\.[a-zA-Z]{2,}\.?$").unwrap();
    static ref LABEL_KEY_NAME_REGEX: Regex =
        Regex::new(r"^[a-z0-9A-Z]([a-z0-9A-Z-_.]*[a-z0-9A-Z]+)?$").unwrap();
}

#[derive(Debug, PartialEq, Snafu)]
pub enum KeyError {
    #[snafu(display("key input cannot be empty"))]
    EmptyInput,

    #[snafu(display("invalid number of slashes in key - expected 0 or 1, got {count}"))]
    InvalidSlashCharCount { count: usize },

    #[snafu(display("failed to parse key prefix"))]
    KeyPrefixError { source: KeyPrefixError },

    #[snafu(display("failed to parse key name"))]
    KeyNameError { source: KeyNameError },
}

/// The [`Key`] of a [`KeyValuePair`](crate::kvp::KeyValuePair). It contains an
/// optional prefix, and a required name. The Kubernetes documentation defines
/// the format and allowed characters [here][k8s-labels]. A [`Key`] is always
/// validated. It also doesn't provide any associated functions which enable
/// unvalidated manipulation of the inner values.
///
/// [k8s-labels]: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
#[derive(Debug, PartialEq)]
pub struct Key {
    prefix: Option<KeyPrefix>,
    name: KeyName,
}

impl FromStr for Key {
    type Err = KeyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();

        // The input cannot be empty
        ensure!(!input.is_empty(), EmptyInputSnafu);

        // Split the input up into the optional prefix and name
        let parts: Vec<_> = input.split('/').collect();

        // Ensure we have 2 or less parts. More parts are a result of too many
        // slashes
        ensure!(
            parts.len() <= 2,
            InvalidSlashCharCountSnafu {
                count: parts.len() - 1
            }
        );

        let (prefix, name) = if parts.len() == 1 {
            (None, KeyName::from_str(parts[0]).context(KeyNameSnafu)?)
        } else {
            (
                Some(KeyPrefix::from_str(parts[0]).context(KeyPrefixSnafu)?),
                KeyName::from_str(parts[1]).context(KeyNameSnafu)?,
            )
        };

        Ok(Self { prefix, name })
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.prefix {
            Some(prefix) => write!(f, "{}/{}", prefix, self.name),
            None => write!(f, "{}", self.name),
        }
    }
}

impl Key {
    /// Retrieves the key's prefix.
    ///
    /// ```
    /// use std::str::FromStr;
    /// use stackable_operator::kvp::{Key, KeyPrefix};
    ///
    /// let key = Key::from_str("stackable.tech/vendor").unwrap();
    /// let prefix = KeyPrefix::from_str("stackable.tech").unwrap();
    ///
    /// assert_eq!(key.prefix(), Some(&prefix));
    /// ```
    pub fn prefix(&self) -> Option<&KeyPrefix> {
        self.prefix.as_ref()
    }

    /// Adds or replaces the key prefix. This takes a parsed and validated
    /// [`KeyPrefix`] as a parameter. If instead you want to use a raw value,
    /// use the [`Key::try_add_prefix()`] function instead.
    pub fn add_prefix(&mut self, prefix: KeyPrefix) {
        self.prefix = Some(prefix)
    }

    /// Adds or replaces the key prefix by parsing and validation raw input. If
    /// instead you already have a parsed and validated [`KeyPrefix`], use the
    /// [`Key::add_prefix()`] function instead.
    pub fn try_add_prefix(&mut self, prefix: impl AsRef<str>) -> Result<&mut Self, KeyError> {
        self.prefix = Some(KeyPrefix::from_str(prefix.as_ref()).context(KeyPrefixSnafu)?);
        Ok(self)
    }

    /// Retrieves the key's name.
    ///
    /// ```
    /// use std::str::FromStr;
    /// use stackable_operator::kvp::{Key, KeyName};
    ///
    /// let key = Key::from_str("stackable.tech/vendor").unwrap();
    /// let name = KeyName::from_str("vendor").unwrap();
    ///
    /// assert_eq!(key.name(), &name);
    /// ```
    pub fn name(&self) -> &KeyName {
        &self.name
    }

    /// Sets the key name. This takes a parsed and validated [`KeyName`] as a
    /// parameter. If instead you want to use a raw value, use the
    /// [`Key::try_set_name()`] function instead.
    pub fn set_name(&mut self, name: KeyName) {
        self.name = name
    }

    /// Sets the key name by parsing and validation raw input. If instead you
    /// already have a parsed and validated [`KeyName`], use the
    /// [`Key::set_name()`] function instead.
    pub fn try_set_name(&mut self, name: impl AsRef<str>) -> Result<&mut Self, KeyError> {
        self.name = KeyName::from_str(name.as_ref()).context(KeyNameSnafu)?;
        Ok(self)
    }
}

#[derive(Debug, PartialEq, Snafu)]
pub enum KeyPrefixError {
    #[snafu(display("prefix segment of key cannot be empty"))]
    PrefixEmpty,

    #[snafu(display("prefix segment of key exceeds the maximum length - expected 253 characters or less, got {length}"))]
    PrefixTooLong { length: usize },

    #[snafu(display("prefix segment of key contains non-ascii characters"))]
    PrefixNotAscii,

    #[snafu(display("prefix segment of key violates kubernetes format"))]
    PrefixInvalid,
}

/// A validated optional [`KeyPrefix`] segment of [`Key`]. Instances of this
/// struct are always valid. [`KeyPrefix`] implements [`Deref`], which enables
/// read-only access to the inner value (a [`String`]). It, however, does not
/// implement [`DerefMut`](std::ops::DerefMut) which would enable unvalidated
/// mutable access to inner values.
#[derive(Debug, PartialEq)]
pub struct KeyPrefix(String);

impl FromStr for KeyPrefix {
    type Err = KeyPrefixError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // The prefix cannot be empty when one is provided
        ensure!(!input.is_empty(), PrefixEmptySnafu);

        // The length of the prefix cannot exceed 253 characters
        ensure!(
            input.len() <= LABEL_KEY_PREFIX_MAX_LEN,
            PrefixTooLongSnafu {
                length: input.len()
            }
        );

        // The prefix cannot contain non-ascii characters
        ensure!(input.is_ascii(), PrefixNotAsciiSnafu);

        // The prefix must use the format specified by Kubernetes
        ensure!(LABEL_KEY_PREFIX_REGEX.is_match(input), PrefixInvalidSnafu);

        Ok(Self(input.to_string()))
    }
}

impl Deref for KeyPrefix {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for KeyPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Snafu)]
pub enum KeyNameError {
    #[snafu(display("name segment of key cannot be empty"))]
    NameEmpty,

    #[snafu(display("name segment of key exceeds the maximum length - expected 63 characters or less, got {length}"))]
    NameTooLong { length: usize },

    #[snafu(display("name segment of key contains non-ascii characters"))]
    NameNotAscii,

    #[snafu(display("name segment of key violates kubernetes format"))]
    NameInvalid,
}

/// A validated [`KeyName`] segment of [`Key`]. This part of the key is
/// required. Instances of this struct are always valid. It also implements
/// [`Deref`], which enables read-only access to the inner value (a [`String`]).
/// It, however, does not implement [`DerefMut`](std::ops::DerefMut) which would
/// enable unvalidated mutable access to inner values.
#[derive(Debug, PartialEq)]
pub struct KeyName(String);

impl FromStr for KeyName {
    type Err = KeyNameError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // The name cannot be empty
        ensure!(!input.is_empty(), NameEmptySnafu);

        // The length of the name cannot exceed 63 characters
        ensure!(
            input.len() <= LABEL_KEY_NAME_MAX_LEN,
            NameTooLongSnafu {
                length: input.len()
            }
        );

        // The name cannot contain non-ascii characters
        ensure!(input.is_ascii(), NameNotAsciiSnafu);

        // The name must use the format specified by Kubernetes
        ensure!(LABEL_KEY_NAME_REGEX.is_match(input), NameInvalidSnafu);

        Ok(Self(input.to_string()))
    }
}

impl Deref for KeyName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for KeyName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[test]
    fn key_with_prefix() {
        let key = Key::from_str("stackable.tech/vendor").unwrap();

        assert_eq!(key.prefix, Some(KeyPrefix("stackable.tech".into())));
        assert_eq!(key.name, KeyName("vendor".into()));
        assert_eq!(key.to_string(), "stackable.tech/vendor");
    }

    #[test]
    fn key_without_prefix() {
        let key = Key::from_str("vendor").unwrap();

        assert_eq!(key.prefix, None);
        assert_eq!(key.name, KeyName("vendor".into()));
        assert_eq!(key.to_string(), "vendor");
    }

    #[rstest]
    #[case("foo/bar/baz", KeyError::InvalidSlashCharCount { count: 2 })]
    #[case("", KeyError::EmptyInput)]
    fn invalid_key(#[case] input: &str, #[case] error: KeyError) {
        let err = Key::from_str(input).unwrap_err();
        assert_eq!(err, error)
    }

    #[rstest]
    #[case("a".repeat(254), KeyPrefixError::PrefixTooLong { length: 254 })]
    #[case("foo.", KeyPrefixError::PrefixInvalid)]
    #[case("ä", KeyPrefixError::PrefixNotAscii)]
    #[case("", KeyPrefixError::PrefixEmpty)]
    fn invalid_key_prefix(#[case] input: String, #[case] error: KeyPrefixError) {
        let err = KeyPrefix::from_str(&input).unwrap_err();
        assert_eq!(err, error)
    }

    #[rstest]
    #[case("a".repeat(64), KeyNameError::NameTooLong { length: 64 })]
    #[case("foo-", KeyNameError::NameInvalid)]
    #[case("ä", KeyNameError::NameNotAscii)]
    #[case("", KeyNameError::NameEmpty)]
    fn invalid_key_name(#[case] input: String, #[case] error: KeyNameError) {
        let err = KeyName::from_str(&input).unwrap_err();
        assert_eq!(err, error)
    }
}
