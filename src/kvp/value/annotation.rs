use std::{fmt::Display, ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::kvp::ValueExt;

#[derive(Debug, PartialEq, Snafu)]
pub enum AnnotationValueError {
    #[snafu(display("value contains non-utf8 characters"))]
    ValueNotUtf8 { source: std::str::Utf8Error },
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AnnotationValue(String);

impl ValueExt for AnnotationValue {
    type Error = AnnotationValueError;
}

impl FromStr for AnnotationValue {
    type Err = AnnotationValueError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = std::str::from_utf8(input.as_bytes()).context(ValueNotUtf8Snafu)?;
        Ok(Self(input.to_owned()))
    }
}

impl Deref for AnnotationValue {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for AnnotationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
