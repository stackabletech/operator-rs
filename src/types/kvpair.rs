use std::{fmt::Display, str::FromStr};

use crate::validation;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum KeyValuePairParseError {
    #[error("invalid key/value pair syntax, expected 'key=value'")]
    InvalidSyntax,

    #[error("key/value pair input cannot be empty")]
    EmptyInput,

    #[error("key/value pair key parse error")]
    KeyParseError(#[from] KeyParseError),
}

#[derive(Debug, PartialEq)]
pub struct KeyValuePair {
    pub key: Key,
    pub value: String,
}

impl FromStr for KeyValuePair {
    type Err = KeyValuePairParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();

        if input.is_empty() {
            return Err(KeyValuePairParseError::EmptyInput);
        }

        let parts: Vec<_> = input.split('=').collect();

        if parts.len() != 2 {
            return Err(KeyValuePairParseError::InvalidSyntax);
        }

        if parts[0].is_empty() || parts[1].is_empty() {
            return Err(KeyValuePairParseError::InvalidSyntax);
        }

        Ok(Self {
            key: Key::from_str(parts[0])?,
            value: parts[1].to_string(),
        })
    }
}

impl TryFrom<&str> for KeyValuePair {
    type Error = KeyValuePairParseError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::from_str(input)
    }
}

impl Display for KeyValuePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl KeyValuePair {
    /// Creates a new key/value pair. The key consists of an optional `prefix`
    /// and a `name`.
    ///
    /// ```
    /// use stackable_operator::types::KeyValuePair;
    ///
    /// // stackable.tech/release=23.7
    /// let kvp = KeyValuePair::new(Some("stackable.tech"), "release", "23.7");
    /// ```
    pub fn new<T>(prefix: Option<T>, name: T, value: T) -> Result<Self, KeyValuePairParseError>
    where
        T: Into<String>,
    {
        let key = Key::new(prefix, name)?;
        let value = value.into();

        Ok(Self { key, value })
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum KeyParseError {
    #[error("invalid key separator count - expected at most '1', got '{0}'")]
    InvalidSeparatorCount(usize),

    #[error("key name is not a valid RFC1123 label")]
    InvalidRfc1123LabelError(Vec<String>),

    #[error("key prefix is not a valid RFC1123 subdomain")]
    InvalidRfc1123SubdomainError(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub struct Key {
    pub prefix: Option<String>,
    pub name: String,
}

impl FromStr for Key {
    type Err = KeyParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = input.trim().split('/').collect();
        let len = parts.len();

        // More then one slash (more than two parts)
        if len > 2 {
            return Err(KeyParseError::InvalidSeparatorCount(len - 1));
        }

        // No prefix, just validate the name segment
        if len == 1 {
            validation::is_rfc_1123_label(parts[0])
                .map_err(KeyParseError::InvalidRfc1123LabelError)?;

            return Ok(Self {
                prefix: None,
                name: parts[0].to_string(),
            });
        }

        // With prefix, validate both
        validation::is_rfc_1123_subdomain(parts[0])
            .map_err(KeyParseError::InvalidRfc1123SubdomainError)?;

        validation::is_rfc_1123_label(parts[1]).map_err(KeyParseError::InvalidRfc1123LabelError)?;

        Ok(Self {
            prefix: Some(parts[0].to_string()),
            name: parts[1].to_string(),
        })
    }
}

impl TryFrom<&str> for Key {
    type Error = KeyParseError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::from_str(input)
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
    pub fn new<T>(prefix: Option<T>, name: T) -> Result<Self, KeyParseError>
    where
        T: Into<String>,
    {
        let prefix = prefix.map(Into::into);
        let name = name.into();

        if let Some(prefix) = &prefix {
            validation::is_rfc_1123_label(prefix)
                .map_err(KeyParseError::InvalidRfc1123LabelError)?;
        }

        validation::is_rfc_1123_label(&name).map_err(KeyParseError::InvalidRfc1123LabelError)?;

        Ok(Self { prefix, name })
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
        let label = KeyValuePair::from_str(input).unwrap();

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
        let result = KeyValuePair::from_str(input);
        assert!(result.is_err());
    }

    #[rstest]
    #[case("=prod")]
    #[case("prod")]
    fn parse_label_missing_key(#[case] input: &str) {
        let result = KeyValuePair::from_str(input);
        assert!(result.is_err());
    }

    #[rstest]
    #[case("stackable.tech/env=prod=invalid")]
    #[case("env=prod=invalid")]
    #[case("=prod=invalid")]
    fn parse_label_too_many_equal_signs(#[case] input: &str) {
        let result = KeyValuePair::from_str(input);
        assert!(result.is_err());
    }

    #[rstest]
    fn parse_label_too_long(long_prefix: &str, long_name: &str) {
        let result = KeyValuePair::from_str(long_prefix);
        assert!(result.is_err());

        let result = KeyValuePair::from_str(long_name);
        assert!(result.is_err());
    }

    #[rstest]
    #[case(" = ")]
    #[case("  ")]
    #[case("")]
    fn parse_label_empty_input(#[case] input: &str) {
        let result = KeyValuePair::from_str(input);
        assert!(result.is_err());
    }
}
