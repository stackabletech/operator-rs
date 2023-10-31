use std::{fmt::Display, ops::Deref, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;
use snafu::{ensure, Snafu};

const LABEL_VALUE_MAX_LEN: usize = 63;

lazy_static! {
    static ref LABEL_VALUE_REGEX: Regex =
        Regex::new(r"^[a-z0-9A-Z]([a-z0-9A-Z-_.]*[a-z0-9A-Z]+)?$").unwrap();
}

#[derive(Debug, Snafu)]
pub enum ValueError {
    #[snafu(display(
        "value exceeds the maximum length - expected 63 characters or less, got {length}"
    ))]
    ValueTooLong { length: usize },

    #[snafu(display("value contains non-ascii characters"))]
    ValueNotAscii,

    #[snafu(display("value violates kubernetes format"))]
    ValueInvalid,
}

pub struct Value(String);

impl FromStr for Value {
    type Err = ValueError;

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

impl Deref for Value {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
