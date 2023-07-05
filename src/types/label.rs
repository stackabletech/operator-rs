use std::{fmt::Display, str::FromStr};

use crate::types::{KeyParseError, KeyValuePair, KeyValuePairExt, KeyValuePairParseError};

pub type LabelParseError = KeyValuePairParseError;
pub type LabelKeyParseError = KeyParseError;

/// Labels are key/value pairs attached to K8s objects like pods. It is modeled
/// after the following [specs][1] It is highly recommended to use [`Label::new`]
/// to create a new label. This method ensures that no maximum length restrictions
/// are violated. [`Label`] also implements [`FromStr`], which allows parsing a
/// label from a string.
///
/// Additionally, [`Label`] implements [`Display`], which formats the label using
/// the following format: `(<prefix>/)<name>=<value>`.
///
/// ### Examples
///
/// ```
/// use stackable_operator::types::Label;
/// use std::str::FromStr;
///
/// let label = Label::new(Some("stackable.tech"), "release", "23.7");
/// let label = Label::from_str("stackable.tech/release=23.7").unwrap();
/// let label = Label::try_from("stackable.tech/release=23.7").unwrap();
/// ```
///
/// [1]: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
#[derive(Clone, Debug)]
pub struct Label(KeyValuePair);

impl KeyValuePairExt for Label {
    /// Creates a new label (key/value pair). The key consists of an optional
    /// `prefix` and a name.
    ///
    /// ```
    /// use stackable_operator::types::Label;
    ///
    /// // stackable.tech/release=23.7
    /// let label = Label::new(Some("stackable.tech"), "release", "23.7");
    /// ```
    fn new<T>(prefix: Option<T>, name: T, value: T) -> Result<Self, LabelParseError>
    where
        T: Into<String>,
    {
        let kvp = KeyValuePair::new(prefix, name, value)?;
        Ok(Self(kvp))
    }

    /// Returns the label key as a formatted [`String`]. If the key contains
    /// a prefix, the key has a format like `<prefix>/<name>`. If not, the key
    /// only consists of a name.
    fn key(&self) -> String {
        self.0.key()
    }

    /// Returns the label value as a [`String`].
    fn value(&self) -> &String {
        self.0.value()
    }
}

impl FromStr for Label {
    type Err = LabelParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let kvp = KeyValuePair::from_str(input)?;
        Ok(Self(kvp))
    }
}

impl TryFrom<&str> for Label {
    type Error = LabelParseError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::from_str(input)
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
