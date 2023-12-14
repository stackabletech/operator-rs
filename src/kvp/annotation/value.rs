use std::{convert::Infallible, fmt::Display, ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::kvp::Value;

/// A validated Kubernetes annotation value, which only requires valid UTF-8
/// data.
///
/// Since [`str`] and [`String`] are guaranteed to be valid UTF-8 data, we
/// don't perform any additional validation.
///
/// This wrapper type solely exists to mirror the label value type.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AnnotationValue(String);

impl Value for AnnotationValue {
    type Error = Infallible;
}

impl FromStr for AnnotationValue {
    type Err = Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self(input.to_owned()))
    }
}

impl Deref for AnnotationValue {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for AnnotationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
