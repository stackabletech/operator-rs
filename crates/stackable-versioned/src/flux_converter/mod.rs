//! `flux-converter` is part of the project DeLorean :)
//!
//! It converts between different CRD versions by using 1.21 GW of power,
//! 142km/h and time travel.

use std::{error::Error, fmt::Write};

use snafu::Snafu;

use crate::ParseResourceVersionError;

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

// We can not put this behind `#[cfg(test)]`, as it seems like the `test` flag is not enabled, when
// a *dependant* crate compiles tests.
pub mod test_utils {
    const TEST_CONVERSION_UUID: &str = "9980028f-816b-4b38-a521-5f087266f76c";

    use kube::{
        api::TypeMeta,
        core::{
            conversion::{ConversionRequest, ConversionReview},
            response::StatusSummary,
        },
    };
    use serde::Serialize;

    pub trait RoundtripTestData: Sized + Serialize {
        fn get_roundtrip_test_data() -> Vec<Self>;
    }

    /// Tests a roundtrip `start_version` -> `middle_version` -> `start_version` and asserts that it
    /// produces the same output as input.
    pub fn test_roundtrip<StartVersion: RoundtripTestData>(
        kind: &str,
        start_version: &str,
        middle_version: &str,
        convert_fn: fn(ConversionReview) -> ConversionReview,
    ) {
        // Construct test data
        let original_specs = StartVersion::get_roundtrip_test_data()
            .iter()
            .map(|spec| {
                serde_json::to_value(spec).expect("Failed to serialize inout roundtrip data")
            })
            .collect::<Vec<_>>();
        let original_objects = specs_to_objects(original_specs.clone(), start_version, kind);

        // Downgrade to the middle version
        let downgrade_conversion_review = conversion_review(original_objects, middle_version);
        let downgraded = convert_fn(downgrade_conversion_review);
        let downgraded_specs = specs_from_conversion_review(downgraded);

        // Upgrade to start version again
        let downgraded_objects = specs_to_objects(downgraded_specs, middle_version, kind);
        let upgrade_conversion_review = conversion_review(downgraded_objects, start_version);
        let upgraded = convert_fn(upgrade_conversion_review);
        let upgraded_specs = specs_from_conversion_review(upgraded);

        // Assert the same output as input
        assert_eq!(upgraded_specs.len(), original_specs.len());
        assert_eq!(
            upgraded_specs, original_specs,
            "The object spec must be the same before and after the roundtrip!"
        );
    }

    fn conversion_review(
        objects: impl IntoIterator<Item = serde_json::Value>,
        desired_api_version: impl Into<String>,
    ) -> ConversionReview {
        let conversion_request = ConversionRequest {
            types: Some(conversion_types()),
            uid: TEST_CONVERSION_UUID.to_string(),
            desired_api_version: desired_api_version.into(),
            objects: objects.into_iter().collect(),
        };
        ConversionReview {
            types: conversion_types(),
            request: Some(conversion_request),
            response: None,
        }
    }

    fn specs_to_objects(
        specs: impl IntoIterator<Item = serde_json::Value>,
        api_version: &str,
        kind: &str,
    ) -> Vec<serde_json::Value> {
        specs
            .into_iter()
            .map(|spec| {
                serde_json::json!({
                    "apiVersion": api_version,
                    "kind": kind,
                    "spec": spec
                })
            })
            .collect()
    }

    fn specs_from_conversion_review(conversion_review: ConversionReview) -> Vec<serde_json::Value> {
        let conversion_result = conversion_review
            .response
            .expect("The ConversionReview needs to have a result");

        assert_eq!(
            conversion_result.result.status,
            Some(StatusSummary::Success),
            "The conversion failed: {conversion_result:?}"
        );

        objects_to_specs(conversion_result.converted_objects)
    }

    fn objects_to_specs(
        objects: impl IntoIterator<Item = serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        objects
            .into_iter()
            .map(|obj| {
                obj.get("spec")
                    .expect("The downgraded objects need to have a spec")
                    .to_owned()
            })
            .collect()
    }

    fn conversion_types() -> TypeMeta {
        TypeMeta {
            api_version: "apiextensions.k8s.io/v1".to_string(),
            kind: "ConversionReview".to_string(),
        }
    }
}
