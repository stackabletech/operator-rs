use std::{fmt::Display, str::FromStr};

use crate::types::{KeyParseError, KeyValuePair, KeyValuePairParseError};

pub type AnnotationParseError = KeyValuePairParseError;
pub type AnnotationKeyParseError = KeyParseError;

/// Annotations are used to attach arbitrary non-identifying metadata to objects,
/// like pods. They are modeled after the following [specs][1]. It is highly
/// recommended to use [`Annotation::new`] to create a new label. This method
/// ensures that no maximum length restrictions are violated. [`Annotation`]
/// also implements [`FromStr`], which allows parsing a annotation from a
/// string.
///
/// Additionally, [`Annotation`] implements [`Display`], which formats the
/// label using the following format: `(<prefix>/)<name>=<value>`.
///
/// ### Examples
///
/// ```
/// use stackable_operator::types::Annotation;
/// use std::str::FromStr;
///
/// let label = Annotation::new(Some("stackable.tech"), "node", "1");
/// let label = Annotation::from_str("stackable.tech/node=1").unwrap();
/// let label = Annotation::try_from("stackable.tech/node=1").unwrap();
/// ```
///
/// [1]: https://kubernetes.io/docs/concepts/overview/working-with-objects/annotations/
pub struct Annotation(KeyValuePair);

impl FromStr for Annotation {
    type Err = AnnotationParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let kvp = KeyValuePair::from_str(input)?;
        Ok(Self(kvp))
    }
}

impl TryFrom<&str> for Annotation {
    type Error = AnnotationParseError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::from_str(input)
    }
}

impl Display for Annotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Annotation {
    /// Creates a new label (key/value pair). The key consists of an optional
    /// `prefix` and a name.
    ///
    /// ```
    /// use stackable_operator::types::Annotation;
    ///
    /// // stackable.tech/node=1
    /// let annotattion = Annotation::new(Some("stackable.tech"), "node", "1");
    /// ```
    pub fn new<T>(prefix: Option<T>, name: T, value: T) -> Result<Self, AnnotationParseError>
    where
        T: Into<String>,
    {
        let kvp = KeyValuePair::new(prefix, name, value)?;
        Ok(Self(kvp))
    }
}
