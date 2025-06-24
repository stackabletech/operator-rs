use std::{fs::File, path::Path};

use insta::{assert_snapshot, glob};
use kube::{
    CustomResource,
    core::{conversion::ConversionReview, response::StatusSummary},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

#[versioned(
    k8s(group = "test.stackable.tech",),
    version(name = "v1alpha1"),
    version(name = "v1alpha2"),
    version(name = "v1beta1"),
    version(name = "v2"),
    version(name = "v3")
)]
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    CustomResource,
    Deserialize,
    JsonSchema,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
struct PersonSpec {
    username: String,

    // In v1alpha2 first and last name have been added
    #[versioned(added(since = "v1alpha2"))]
    first_name: String,
    #[versioned(added(since = "v1alpha2"))]
    last_name: String,

    // We started out with a enum. As we *need* to provide a default, we have a Unknown variant.
    // Afterwards we figured let's be more flexible and accept any arbitrary String.
    #[versioned(
        added(since = "v2", default = "default_gender"),
        changed(since = "v3", from_type = "Gender")
    )]
    gender: String,
}

fn default_gender() -> Gender {
    Gender::Unknown
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, JsonSchema, Serialize,
)]
#[serde(rename_all = "PascalCase")]
pub enum Gender {
    Unknown,
    Male,
    Female,
}

impl From<Gender> for String {
    fn from(value: Gender) -> Self {
        match value {
            Gender::Unknown => "Unknown".to_owned(),
            Gender::Male => "Male".to_owned(),
            Gender::Female => "Female".to_owned(),
        }
    }
}

impl From<String> for Gender {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Male" => Self::Male,
            "Female" => Self::Female,
            _ => Self::Unknown,
        }
    }
}

#[test]
fn pass() {
    glob!("./inputs/conversions/pass/", "*.json", |path| {
        let (request, response) = run_for_file(path);

        let formatted = serde_json::to_string_pretty(&response)
            .expect("Failed to serialize ConversionResponse");
        assert_snapshot!(formatted);

        let response = response
            .response
            .expect("ConversionReview had no response!");

        assert_eq!(
            response.result.status,
            Some(StatusSummary::Success),
            "File {path:?} should be converted successfully"
        );
        assert_eq!(request.request.unwrap().uid, response.uid);
    })
}

#[test]
fn fail() {
    glob!("./inputs/conversions/fail/", "*.json", |path| {
        let (request, response) = run_for_file(path);

        let formatted = serde_json::to_string_pretty(&response)
            .expect("Failed to serialize ConversionResponse");
        assert_snapshot!(formatted);

        let response = response
            .response
            .expect("ConversionReview had no response!");

        assert_eq!(
            response.result.status,
            Some(StatusSummary::Failure),
            "File {path:?} should *not* be converted successfully"
        );
        if let Some(request) = &request.request {
            assert_eq!(request.uid, response.uid);
        }
    })
}

fn run_for_file(path: &Path) -> (ConversionReview, ConversionReview) {
    let request: ConversionReview =
        serde_json::from_reader(File::open(path).expect("failed to open test file"))
            .expect("failed to parse ConversionReview from test file");
    let response = Person::try_convert(request.clone());

    (request, response)
}
