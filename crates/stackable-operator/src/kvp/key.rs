use std::{fmt::Display, ops::Deref, str::FromStr, sync::LazyLock};

use regex::Regex;
use snafu::{ensure, ResultExt, Snafu};

const KEY_PREFIX_MAX_LEN: usize = 253;
const KEY_NAME_MAX_LEN: usize = 63;

// Lazily initialized regular expressions
static KEY_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z](\.?[a-zA-Z0-9-])*\.[a-zA-Z]{2,}\.?$")
        .expect("failed to compile key prefix regex")
});

static KEY_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9A-Z]([a-z0-9A-Z-_.]*[a-z0-9A-Z]+)?$")
        .expect("failed to compile key name regex")
});

/// The error type for key parsing/validation operations.
///
/// This error will be returned if the input is empty, the parser encounters
/// multiple prefixes or any deeper errors occur during key prefix and key name
/// parsing.
#[derive(Debug, PartialEq, Snafu)]
pub enum KeyError {
    /// Indicates that the input is empty. The key must at least contain a name.
    /// The prefix is optional.
    #[snafu(display("key input cannot be empty"))]
    EmptyInput,

    /// Indicates that the input contains multiple nested prefixes, e.g.
    /// `app.kubernetes.io/nested/name`. Valid keys only contain one prefix
    /// like `app.kubernetes.io/name`.
    #[snafu(display("key prefixes cannot be nested, only use a single slash"))]
    NestedPrefix,

    /// Indicates that the key prefix failed to parse. See [`KeyPrefixError`]
    /// for more information about error causes.
    #[snafu(display("failed to parse key prefix"))]
    KeyPrefixError { source: KeyPrefixError },

    /// Indicates that the key name failed to parse. See [`KeyNameError`] for
    /// more information about error causes.
    #[snafu(display("failed to parse key name"))]
    KeyNameError { source: KeyNameError },
}

/// The key of a a key/value pair. It contains an optional prefix, and a
/// required name.
///
/// The general format is `(<PREFIX>/)<NAME>`. Further, the Kubernetes
/// documentation defines the format and allowed characters in more detail
/// [here][k8s-labels]. A [`Key`] is always validated. It also doesn't provide
/// any associated functions which enable unvalidated manipulation of the inner
/// values.
///
/// [k8s-labels]: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
        let parts = input.split('/').collect::<Vec<_>>();

        let (prefix, name) = match parts[..] {
            [name] => (None, name),
            [prefix, name] => (Some(prefix), name),
            _ => return NestedPrefixSnafu.fail(),
        };

        let key = Self {
            prefix: prefix
                .map(KeyPrefix::from_str)
                .transpose()
                .context(KeyPrefixSnafu)?,
            name: KeyName::from_str(name).context(KeyNameSnafu)?,
        };

        Ok(key)
    }
}

impl TryFrom<&str> for Key {
    type Error = KeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
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

impl From<&Key> for String {
    fn from(value: &Key) -> Self {
        value.to_string()
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

/// The error type for key prefix parsing/validation operations.
#[derive(Debug, PartialEq, Snafu)]
pub enum KeyPrefixError {
    /// Indicates that the key prefix segment is empty, which is not permitted
    /// when the key indicates that a prefix is present (via a slash). This
    /// prevents keys like `/name`.
    #[snafu(display("prefix segment of key cannot be empty"))]
    PrefixEmpty,

    /// Indicates that the key prefix segment exceeds the mamximum length of
    /// 253 ASCII characters. It additionally reports how many characters were
    /// encountered during parsing / validation.
    #[snafu(display("prefix segment of key exceeds the maximum length - expected 253 characters or less, got {length}"))]
    PrefixTooLong { length: usize },

    /// Indidcates that the key prefix segment contains non-ASCII characters
    /// which the Kubernetes spec does not permit.
    #[snafu(display("prefix segment of key contains non-ascii characters"))]
    PrefixNotAscii,

    /// Indicates that the key prefix segment violates the specified Kubernetes
    /// format.
    #[snafu(display("prefix segment of key violates kubernetes format"))]
    PrefixInvalid,
}

/// A validated optional key prefix segment of a key.
///
/// Instances of this struct are always valid. [`KeyPrefix`] implements
/// [`Deref`], which enables read-only access to the inner value (a [`String`]).
/// It, however, does not implement [`DerefMut`](std::ops::DerefMut) which would
/// enable unvalidated mutable access to inner values.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyPrefix(String);

impl FromStr for KeyPrefix {
    type Err = KeyPrefixError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // The prefix cannot be empty when one is provided
        ensure!(!input.is_empty(), PrefixEmptySnafu);

        // The length of the prefix cannot exceed 253 characters
        ensure!(
            input.len() <= KEY_PREFIX_MAX_LEN,
            PrefixTooLongSnafu {
                length: input.len()
            }
        );

        // The prefix cannot contain non-ascii characters
        ensure!(input.is_ascii(), PrefixNotAsciiSnafu);

        // The prefix must use the format specified by Kubernetes
        ensure!(KEY_PREFIX_REGEX.is_match(input), PrefixInvalidSnafu);

        Ok(Self(input.to_string()))
    }
}

impl Deref for KeyPrefix {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for KeyPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> PartialEq<T> for KeyPrefix
where
    T: AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.deref() == other.as_ref()
    }
}

/// The error type for key name parsing/validation operations.
#[derive(Debug, PartialEq, Snafu)]
pub enum KeyNameError {
    /// Indicates that the key name segment is empty. The key name is required
    /// and therefore cannot be empty.
    #[snafu(display("name segment of key cannot be empty"))]
    NameEmpty,

    /// Indicates that the key name sgement exceeds the maximum length of 63
    /// ASCII characters. It additionally reports how many characters were
    /// encountered during parsing / validation.
    #[snafu(display("name segment of key exceeds the maximum length - expected 63 characters or less, got {length}"))]
    NameTooLong { length: usize },

    /// Indidcates that the key name segment contains non-ASCII characters
    /// which the Kubernetes spec does not permit.
    #[snafu(display("name segment of key contains non-ascii characters"))]
    NameNotAscii,

    /// Indicates that the key name segment violates the specified Kubernetes
    /// format.
    #[snafu(display("name segment of key violates kubernetes format"))]
    NameInvalid,
}

/// A validated name segement of a key. This part of the key is required.
///
/// Instances of this struct are always valid. It also implements [`Deref`],
/// which enables read-only access to the inner value (a [`String`]). It,
/// however, does not implement [`DerefMut`](std::ops::DerefMut) which would
/// enable unvalidated mutable access to inner values.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyName(String);

impl FromStr for KeyName {
    type Err = KeyNameError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // The name cannot be empty
        ensure!(!input.is_empty(), NameEmptySnafu);

        // The length of the name cannot exceed 63 characters
        ensure!(
            input.len() <= KEY_NAME_MAX_LEN,
            NameTooLongSnafu {
                length: input.len()
            }
        );

        // The name cannot contain non-ascii characters
        ensure!(input.is_ascii(), NameNotAsciiSnafu);

        // The name must use the format specified by Kubernetes
        ensure!(KEY_NAME_REGEX.is_match(input), NameInvalidSnafu);

        Ok(Self(input.to_string()))
    }
}

impl Deref for KeyName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for KeyName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> PartialEq<T> for KeyName
where
    T: AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.deref() == other.as_ref()
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use crate::kvp::Label;

    use super::*;

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

    #[test]
    fn prefix_equality() {
        const EXAMPLE_PREFIX_STR: &str = "stackable.tech";

        let example_prefix = KeyPrefix::from_str(EXAMPLE_PREFIX_STR).expect("valid test prefix");
        assert!(example_prefix == EXAMPLE_PREFIX_STR);
    }

    #[test]
    fn name_equality() {
        const EXAMPLE_NAME_STR: &str = "managed-by";

        let example_name = KeyName::from_str(EXAMPLE_NAME_STR).expect("valid test name");
        assert!(example_name == EXAMPLE_NAME_STR);
    }

    #[rstest]
    #[case("foo/bar/baz", KeyError::NestedPrefix)]
    #[case("", KeyError::EmptyInput)]
    fn invalid_key(#[case] input: &str, #[case] error: KeyError) {
        let err = Key::from_str(input).unwrap_err();
        assert_eq!(err, error);
    }

    #[rstest]
    #[case("a".repeat(254), KeyPrefixError::PrefixTooLong { length: 254 })]
    #[case("foo.", KeyPrefixError::PrefixInvalid)]
    #[case("ä", KeyPrefixError::PrefixNotAscii)]
    #[case("", KeyPrefixError::PrefixEmpty)]
    fn invalid_key_prefix(#[case] input: String, #[case] error: KeyPrefixError) {
        let err = KeyPrefix::from_str(&input).unwrap_err();
        assert_eq!(err, error);
    }

    #[rstest]
    #[case("a".repeat(64), KeyNameError::NameTooLong { length: 64 })]
    #[case("foo-", KeyNameError::NameInvalid)]
    #[case("ä", KeyNameError::NameNotAscii)]
    #[case("", KeyNameError::NameEmpty)]
    fn invalid_key_name(#[case] input: String, #[case] error: KeyNameError) {
        let err = KeyName::from_str(&input).unwrap_err();
        assert_eq!(err, error);
    }

    #[rstest]
    #[case("app.kubernetes.io/name", true)]
    #[case("name", false)]
    fn key_prefix_deref(#[case] key: &str, #[case] expected: bool) {
        let label = Label::try_from((key, "zookeeper")).unwrap();

        let is_valid = label
            .key
            .prefix()
            .is_some_and(|prefix| *prefix == "app.kubernetes.io");

        assert_eq!(is_valid, expected)
    }

    #[rstest]
    #[case("app.kubernetes.io/name", true)]
    #[case("app.kubernetes.io/foo", false)]
    #[case("name", true)]
    #[case("foo", false)]
    fn key_name_deref(#[case] key: &str, #[case] expected: bool) {
        let label = Label::try_from((key, "zookeeper")).unwrap();
        let is_valid = *label.key.name() == "name";

        assert_eq!(is_valid, expected);
    }
}
