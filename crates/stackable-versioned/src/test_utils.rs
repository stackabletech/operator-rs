use kube::{
    api::TypeMeta,
    core::{
        conversion::{ConversionRequest, ConversionReview},
        response::StatusSummary,
    },
};
use serde::Serialize;

const TEST_CONVERSION_UUID: &str = "9980028f-816b-4b38-a521-5f087266f76c";

/// One very important requirement for CRD conversions is that they support roundtrips, which means
/// that we can not loose data when starting at version A, converting to version B and back to
/// version A.
///
/// As it's very hard to make sure the roundtrips never loos data, the
/// [`crate::versioned`] macro automatically generates tests that test roundtrips.
/// However, for that to work it needs some test data to run through the conversions, hence it
/// requires you to provide test data for the earliest and latest version of the CRD struct.
///
/// You can provide test data e.g. as follows
/// ```
/// #[cfg(test)]
/// impl stackable_versioned::test_utils::RoundtripTestData for v1alpha1::ListenerClassSpec {
///     fn roundtrip_test_data() -> Vec<Self> {
///         stackable_operator::utils::yaml_from_str_singleton_map(indoc::indoc! {"
///           - serviceType: ClusterIP
///           - serviceType: NodePort
///           - serviceType: LoadBalancer
///           - serviceType: ClusterIP
///             loadBalancerAllocateNodePorts: false
///             loadBalancerClass: foo
///             serviceAnnotations:
///               foo: bar
///             serviceExternalTrafficPolicy: Local
///             preferredAddressType: HostnameConservative
///         "})
///         .expect("Failed to parse ListenerClassSpec YAML")
///     }
/// }
/// ```
pub trait RoundtripTestData: Sized + Serialize {
    fn roundtrip_test_data() -> Vec<Self>;
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
    let original_specs = StartVersion::roundtrip_test_data()
        .iter()
        .map(|spec| serde_json::to_value(spec).expect("Failed to serialize inout roundtrip data"))
        .collect::<Vec<_>>();
    let original_objects = specs_to_objects(original_specs.clone(), start_version, kind);

    // Downgrade to the middle version
    let downgrade_conversion_review = conversion_review(original_objects, middle_version);
    let downgraded = convert_fn(downgrade_conversion_review);
    let downgraded_objects = objects_from_conversion_review(downgraded);

    dbg!(&downgraded_objects);

    // Upgrade to start version again
    let upgrade_conversion_review = conversion_review(downgraded_objects, start_version);
    let upgraded = convert_fn(upgrade_conversion_review);
    let upgraded_objects = objects_from_conversion_review(upgraded);
    let upgraded_specs = objects_to_specs(upgraded_objects);

    // Assert the same output as input
    assert_eq!(upgraded_specs.len(), original_specs.len());
    assert_eq!(
        upgraded_specs, original_specs,
        "The object spec must be the same before and after the roundtrip!"
    );
}

/// Crates a [`ConversionReview`] that requests to convert the passed `objects` to the desired
/// apiVersion
fn conversion_review(
    objects: impl IntoIterator<Item = serde_json::Value>,
    desired_api_version: impl Into<String>,
) -> ConversionReview {
    let conversion_request = ConversionRequest {
        types: Some(conversion_type()),
        uid: TEST_CONVERSION_UUID.to_string(),
        desired_api_version: desired_api_version.into(),
        objects: objects.into_iter().collect(),
    };
    ConversionReview {
        types: conversion_type(),
        request: Some(conversion_request),
        response: None,
    }
}

/// Converts from a `.spec` field (as [`serde_json::Value`]) to the entire Kubernetes object
/// (also as [`serde_json::Value`]).
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
                "spec": spec,
                "metadata": {},
            })
        })
        .collect()
}

/// Extracts the `.spec` field out of Kubernetes objects
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

/// Asserts that the [`ConversionReview`] was successful and extracts the `converted_objects`
fn objects_from_conversion_review(conversion_review: ConversionReview) -> Vec<serde_json::Value> {
    let conversion_result = conversion_review
        .response
        .expect("The ConversionReview needs to have a result");

    assert_eq!(
        conversion_result.result.status,
        Some(StatusSummary::Success),
        "The conversion failed: {conversion_result:?}"
    );

    conversion_result.converted_objects
}

fn conversion_type() -> TypeMeta {
    TypeMeta {
        api_version: "apiextensions.k8s.io/v1".to_string(),
        kind: "ConversionReview".to_string(),
    }
}
