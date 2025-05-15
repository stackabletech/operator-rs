use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned_macros::versioned;

use crate as stackable_versioned;

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

impl Into<String> for Gender {
    fn into(self) -> String {
        match self {
            Gender::Unknown => "Unknown".to_string(),
            Gender::Male => "Male".to_string(),
            Gender::Female => "Female".to_string(),
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

/// TEMP, we need to implement downgrades manually
impl From<v1alpha2::PersonSpec> for v1alpha1::PersonSpec {
    fn from(value: v1alpha2::PersonSpec) -> Self {
        Self {
            username: value.username,
        }
    }
}
impl From<v1beta1::PersonSpec> for v1alpha2::PersonSpec {
    fn from(value: v1beta1::PersonSpec) -> Self {
        Self {
            username: value.username,
            first_name: value.first_name,
            last_name: value.last_name,
        }
    }
}
impl From<v2::PersonSpec> for v1beta1::PersonSpec {
    fn from(value: v2::PersonSpec) -> Self {
        Self {
            username: value.username,
            first_name: value.first_name,
            last_name: value.last_name,
        }
    }
}
impl From<v3::PersonSpec> for v2::PersonSpec {
    fn from(value: v3::PersonSpec) -> Self {
        Self {
            username: value.username,
            first_name: value.first_name,
            last_name: value.last_name,
            gender: value.gender.into(),
        }
    }
}
/// END TEMP

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path};

    use insta::{assert_snapshot, glob};
    use kube::core::{
        conversion::{ConversionResponse, ConversionReview},
        response::StatusSummary,
    };

    use super::Person;

    #[test]
    fn pass() {
        glob!("../../../fixtures/inputs/pass/", "*.json", |path| {
            let (review, response) = run_for_file(path);

            assert_eq!(response.result.status, Some(StatusSummary::Success));
            assert_eq!(review.request.unwrap().uid, response.uid);

            let formatted = serde_json::to_string_pretty(&response)
                .expect("Failed to serialize ConversionResponse");
            assert_snapshot!(formatted);
        })
    }

    #[test]
    fn fail() {
        glob!("../../../fixtures/inputs/fail/", "*.json", |path| {
            let (review, response) = run_for_file(path);

            assert_eq!(response.result.status, Some(StatusSummary::Failure));
            if let Some(request) = &review.request {
                assert_eq!(request.uid, response.uid);
            }

            let formatted = serde_json::to_string_pretty(&response)
                .expect("Failed to serialize ConversionResponse");
            assert_snapshot!(formatted);
        })
    }

    fn run_for_file(path: &Path) -> (ConversionReview, ConversionResponse) {
        let review: ConversionReview =
            serde_json::from_reader(File::open(path).expect("failed to open test file"))
                .expect("failed to parse ConversionReview from test file");
        let response = Person::convert(review.clone());

        (review, response)
    }
}
