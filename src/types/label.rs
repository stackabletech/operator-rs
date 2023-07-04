use std::{fmt::Display, str::FromStr};

use crate::validation;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LabelParseError {
    #[error("invalid label syntax, expected 'key=value'")]
    InvalidSyntax,

    #[error("label input cannot be empty")]
    EmptyInput,

    #[error("label key parse error")]
    LabelKeyParseError(#[from] LabelKeyParseError),
}

#[derive(Debug, thiserror::Error)]
pub enum LabelCreateError {
    #[error("label name is not a valid RFC1123 label")]
    InvalidRfc1123LabelError(Vec<String>),

    #[error("label prefix is not a valid RFC1123 subdomain")]
    InvalidRfc1123SubdomainError(Vec<String>),
}

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
#[derive(Debug, PartialEq)]
pub struct Label {
    pub key: LabelKey,
    pub value: String,
}

impl FromStr for Label {
    type Err = LabelParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();

        if input.is_empty() {
            return Err(LabelParseError::EmptyInput);
        }

        let parts: Vec<_> = input.split('=').collect();

        if parts.len() != 2 {
            return Err(LabelParseError::InvalidSyntax);
        }

        if parts[0].is_empty() || parts[1].is_empty() {
            return Err(LabelParseError::InvalidSyntax);
        }

        Ok(Self {
            key: LabelKey::from_str(parts[0])?,
            value: parts[1].to_string(),
        })
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
        write!(f, "{}={}", self.key, self.value)
    }
}

impl Label {
    /// Creates a new label (key/value pair). The key consists of an optional
    /// `prefix` and a name.
    ///
    /// ```
    /// use stackable_operator::types::Label;
    ///
    /// // stackable.tech/release=23.7
    /// let label = Label::new(Some("stackable.tech"), "release", "23.7");
    /// ```
    pub fn new<T>(prefix: Option<T>, name: T, value: T) -> Result<Self, LabelCreateError>
    where
        T: Into<String>,
    {
        let prefix = prefix.map(Into::into);
        let value = value.into();
        let name = name.into();

        if let Some(prefix) = &prefix {
            validation::is_rfc_1123_label(&prefix)
                .map_err(LabelCreateError::InvalidRfc1123LabelError)?;
        }

        validation::is_rfc_1123_label(&name).map_err(LabelCreateError::InvalidRfc1123LabelError)?;

        Ok(Self {
            key: LabelKey { prefix, name },
            value,
        })
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LabelKeyParseError {
    #[error("invalid label separator count - expected at most '1', got '{0}'")]
    InvalidSeparatorCount(usize),

    #[error("label name is not a valid RFC1123 label")]
    InvalidRfc1123LabelError(Vec<String>),

    #[error("label prefix is not a valid RFC1123 subdomain")]
    InvalidRfc1123SubdomainError(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub struct LabelKey {
    pub prefix: Option<String>,
    pub name: String,
}

impl FromStr for LabelKey {
    type Err = LabelKeyParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = input.trim().split('/').collect();
        let len = parts.len();

        // More then one slash (more than two parts)
        if len > 2 {
            return Err(LabelKeyParseError::InvalidSeparatorCount(len - 1));
        }

        // No prefix, just validate the name segment
        if len == 1 {
            validation::is_rfc_1123_label(parts[0])
                .map_err(LabelKeyParseError::InvalidRfc1123LabelError)?;

            return Ok(Self {
                prefix: None,
                name: parts[0].to_string(),
            });
        }

        // With prefix, validate both
        validation::is_rfc_1123_subdomain(parts[0])
            .map_err(LabelKeyParseError::InvalidRfc1123SubdomainError)?;

        validation::is_rfc_1123_label(parts[1])
            .map_err(LabelKeyParseError::InvalidRfc1123LabelError)?;

        Ok(Self {
            prefix: Some(parts[0].to_string()),
            name: parts[1].to_string(),
        })
    }
}

impl TryFrom<&str> for LabelKey {
    type Error = LabelKeyParseError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::from_str(input)
    }
}

impl Display for LabelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.prefix {
            Some(prefix) => write!(f, "{}/{}", prefix, self.name),
            None => write!(f, "{}", self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[fixture]
    fn long_prefix<'a>() -> &'a str {
        "this.is.my.super.long.label.prefix.which.should.break.this.test.case.with.ease.but.this.is.super.tedious.to.write.out.maaaaaaaaaaaaan.this.is.getting.ridiculous.why.are.such.long.domains.even.allowed.who.uses.these.for.real.and.we.are.done.and.reached.the.maximum.length/env=prod"
    }

    #[fixture]
    fn long_name<'a>() -> &'a str {
        "stackable.tech/this.is.a.super.loooooooooong.label.name.which.is.way.too.long.to.be.valid"
    }

    #[rstest]
    #[case("stackable.tech/env=prod", "stackable.tech/env", "prod")]
    #[case("env=prod", "env", "prod")]
    fn parse_label_valid(#[case] input: &str, #[case] key: &str, #[case] value: &str) {
        let label = Label::from_str(input).unwrap();

        assert_eq!(label.key.to_string(), key.to_string());
        assert_eq!(label.value, value.to_string());
        assert_eq!(label.to_string(), input.to_string());
    }

    #[rstest]
    #[case("stackable.tech/env=")]
    #[case("stackable.tech/env")]
    #[case("env=")]
    #[case("env")]
    fn parse_label_missing_value(#[case] input: &str) {
        let result = Label::from_str(input);
        assert!(result.is_err());
    }

    #[rstest]
    #[case("=prod")]
    #[case("prod")]
    fn parse_label_missing_key(#[case] input: &str) {
        let result = Label::from_str(input);
        assert!(result.is_err());
    }

    #[rstest]
    #[case("stackable.tech/env=prod=invalid")]
    #[case("env=prod=invalid")]
    #[case("=prod=invalid")]
    fn parse_label_too_many_equal_signs(#[case] input: &str) {
        let result = Label::from_str(input);
        assert!(result.is_err());
    }

    #[rstest]
    fn parse_label_too_long(long_prefix: &str, long_name: &str) {
        let result = Label::from_str(long_prefix);
        assert!(result.is_err());

        let result = Label::from_str(long_name);
        assert!(result.is_err());
    }

    #[rstest]
    #[case(" = ")]
    #[case("  ")]
    #[case("")]
    fn parse_label_empty_input(#[case] input: &str) {
        let result = Label::from_str(input);
        assert!(result.is_err());
    }
}
