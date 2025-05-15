//! `flux-converter` is part of the project DeLorean :)
//!
//! It converts between different CRD versions by using 1.21 GW of power,
//! 142km/h and time travel.

use kube::core::conversion::ConvertConversionReviewError;
use snafu::Snafu;

#[cfg(test)]
mod tests;

#[derive(Debug, Snafu)]
pub enum ParseResourceVersionError {
    #[snafu(display("the resource version \"{version}\" is not known"))]
    UnknownResourceVersion { version: String },
}

#[derive(Debug, Snafu)]
pub enum ConversionError {
    #[snafu(display("failed to convert ConversionReview to ConversionRequest"))]
    ConvertReviewToRequest {
        source: ConvertConversionReviewError,
    },

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
        "I was asked to convert the kind \"{expected_kind}\", but I can only convert objects of kind \"{send_kind}\""
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
