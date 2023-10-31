use std::{fmt::Display, str::FromStr};

use snafu::{ensure, ResultExt, Snafu};

mod key;
mod value;

pub use key::*;
pub use value::*;

#[derive(Debug, Snafu)]
pub enum KeyPairError {
    #[snafu(display("label input cannot be empty"))]
    EmptyInput,

    #[snafu(display("invalid number of equal signs - expected exactly 1, got {signs}"))]
    InvalidEqualSignCount { signs: usize },

    #[snafu(display("failed to parse label key"))]
    KeyError { source: KeyError },

    #[snafu(display("failed to parse label value"))]
    ValueError { source: ValueError },
}

pub struct KeyValuePair {
    key: Key,
    value: Value,
}

impl FromStr for KeyValuePair {
    type Err = KeyPairError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();

        // Ensure the input is not empty
        ensure!(!input.is_empty(), EmptyInputSnafu);

        // Then split up the key and value, which is separated by an equal
        // sign
        let parts: Vec<_> = input.split('=').collect();

        // Ensure there are only two parts
        ensure!(
            parts.len() == 2,
            InvalidEqualSignCountSnafu { signs: parts.len() }
        );

        // Parse key and value parts
        let key = Key::from_str(parts[0]).context(KeySnafu)?;
        let value = Value::from_str(parts[1]).context(ValueSnafu)?;

        Ok(Self { key, value })
    }
}

impl Display for KeyValuePair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl KeyValuePair {
    pub fn new(key: Key, value: Value) -> Self {
        Self { key, value }
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}
