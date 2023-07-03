use std::{fmt::Display, str::FromStr};

use crate::constants::validation::{RFC_1123_LABEL_MAX_LENGTH, RFC_1123_SUBDOMAIN_MAX_LENGTH};

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LabelParseError {
    #[error("invalid label syntax, expected 'key=value'")]
    InvalidSyntax,

    #[error("label key parse error")]
    LabelKeyParseError(#[from] LabelKeyParseError),
}

/// Labels are key/value pairs attached to K8s objects like pods.
/// It is modeled after the following specs: https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/
#[derive(Debug, PartialEq)]
pub struct Label {
    pub key: LabelKey,
    pub value: String,
}

impl FromStr for Label {
    type Err = LabelParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = input.trim().split("=").collect();

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

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LabelKeyParseError {
    #[error("invalid label separator count - expected at most '1', got '{0}'")]
    InvalidSeparatorCount(usize),

    #[error("invalid label key name length - expected '63' characters max, got {0}")]
    InvalidKeyNameLength(usize),

    #[error("invalid label key prefix length - expected '253' characters max, got {0}")]
    InvalidKeyPrefixLength(usize),
}

#[derive(Debug, PartialEq)]
pub struct LabelKey {
    pub prefix: Option<String>,
    pub name: String,
}

impl FromStr for LabelKey {
    type Err = LabelKeyParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = input.trim().split("/").collect();
        let len = parts.len();

        // More then one slash (more than two parts)
        if len > 2 {
            return Err(LabelKeyParseError::InvalidSeparatorCount(len - 1));
        }

        // No prefix, just parse the name segment
        if len == 1 {
            if parts[0].len() > RFC_1123_LABEL_MAX_LENGTH {
                return Err(LabelKeyParseError::InvalidKeyNameLength(parts[0].len()));
            }

            return Ok(Self {
                prefix: None,
                name: parts[0].to_string(),
            });
        }

        // With prefix, parse both
        if parts[0].len() > RFC_1123_SUBDOMAIN_MAX_LENGTH {
            return Err(LabelKeyParseError::InvalidKeyPrefixLength(parts[0].len()));
        }

        if parts[1].len() > RFC_1123_LABEL_MAX_LENGTH {
            return Err(LabelKeyParseError::InvalidKeyNameLength(parts[1].len()));
        }

        Ok(Self {
            prefix: Some(parts[0].to_string()),
            name: parts[1].to_string(),
        })
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

    #[test]
    fn parse_label_valid() {
        let input = "environment=production";
        let label = Label::from_str(input).unwrap();

        assert_eq!(
            label.key,
            LabelKey {
                prefix: None,
                name: "environment".into()
            }
        );

        assert_eq!(label.value, "production".to_string());
    }

    #[test]
    fn parse_label_missing_value() {
        let input = "environment=";
        let result = Label::from_str(input);

        assert_eq!(result, Err(LabelParseError::InvalidSyntax));

        let input = "environment";
        let result = Label::from_str(input);

        assert_eq!(result, Err(LabelParseError::InvalidSyntax));
    }
}
