//! `flux-converter` is part of the project DeLorean :)
//!
//! It converts between different CRD versions by using 1.21 GW of power,
//! 142km/h and time travel.

use std::{error::Error, fmt::Write};

use snafu::Snafu;

use crate::ParseResourceVersionError;

mod apply_crd;

#[cfg(test)]
mod tests;

#[derive(Debug, Snafu)]
pub enum ConversionError {
    #[snafu(display("failed to parse current resource version \"{version}\""))]
    ParseCurrentResourceVersion {
        source: ParseResourceVersionError,
        version: String,
    },

    #[snafu(display("failed to parse desired resource version \"{version}\""))]
    ParseDesiredResourceVersion {
        source: ParseResourceVersionError,
        version: String,
    },

    #[snafu(display("the object send for conversion has no \"spec\" field"))]
    ObjectHasNoSpec {},

    #[snafu(display("the object send for conversion has no \"kind\" field"))]
    ObjectHasNoKind {},

    #[snafu(display("the object send for conversion has no \"apiVersion\" field"))]
    ObjectHasNoApiVersion {},

    #[snafu(display("the \"kind\" field of the object send for conversion isn't a String"))]
    ObjectKindNotString { kind: serde_json::Value },

    #[snafu(display("the \"apiVersion\" field of the object send for conversion isn't a String"))]
    ObjectApiVersionNotString { api_version: serde_json::Value },

    #[snafu(display(
        "I was asked to convert the kind \"{send_kind}\", but I can only convert objects of kind \"{expected_kind}\""
    ))]
    WrongObjectKind {
        expected_kind: String,
        send_kind: String,
    },

    #[snafu(display("failed to deserialize object of kind \"{kind}\""))]
    DeserializeObjectSpec {
        source: serde_json::Error,
        kind: String,
    },

    #[snafu(display("failed to serialize object of kind \"{kind}\""))]
    SerializeObjectSpec {
        source: serde_json::Error,
        kind: String,
    },
}

impl ConversionError {
    pub fn http_return_code(&self) -> u16 {
        match &self {
            ConversionError::ParseCurrentResourceVersion { .. } => 500,
            ConversionError::ParseDesiredResourceVersion { .. } => 500,
            ConversionError::ObjectHasNoSpec {} => 400,
            ConversionError::ObjectHasNoKind {} => 400,
            ConversionError::ObjectHasNoApiVersion {} => 400,
            ConversionError::ObjectKindNotString { .. } => 400,
            ConversionError::ObjectApiVersionNotString { .. } => 400,
            ConversionError::WrongObjectKind { .. } => 400,
            ConversionError::DeserializeObjectSpec { .. } => 500,
            ConversionError::SerializeObjectSpec { .. } => 500,
        }
    }

    pub fn as_human_readable_error_message(&self) -> String {
        let mut error_message = String::new();
        write!(error_message, "{self}").expect("Writing to Strings can not fail");

        let mut source = self.source();
        while let Some(err) = source {
            write!(error_message, ": {err}").expect("Writing to Strings can not fail");
            source = err.source();
        }

        error_message
    }
}
