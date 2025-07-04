use std::{fs::File, path::Path};

use kube::{
    CustomResource,
    core::conversion::{ConversionRequest, ConversionReview},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

// Fixes an error with this function being marked as unused. See
// - https://stackoverflow.com/a/67902444
// - https://github.com/rust-lang/rust/issues/46379
#[allow(dead_code)]
pub fn convert_via_file(path: &Path) -> (ConversionReview, ConversionReview) {
    let request: ConversionReview =
        serde_json::from_reader(File::open(path).expect("failed to open test file"))
            .expect("failed to parse ConversionReview from test file");
    let response = Person::try_convert(request.clone());

    (request, response)
}

#[allow(dead_code)]
pub fn roundtrip_conversion_review(
    response_review: ConversionReview,
    desired_api_version: PersonVersion,
) -> ConversionReview {
    let response = response_review.response.unwrap();
    ConversionReview {
        types: response_review.types,
        request: Some(ConversionRequest {
            desired_api_version: desired_api_version.as_api_version_str().to_owned(),
            objects: response.converted_objects,
            types: response.types,
            uid: response.uid,
        }),
        response: None,
    }
}

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1alpha2"),
    version(name = "v1beta1"),
    version(name = "v2"),
    version(name = "v3"),
    options(k8s(experimental_conversion_tracking))
)]
pub mod versioned {
    #[versioned(crd(group = "test.stackable.tech"))]
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
    pub struct PersonSpec {
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

        #[versioned(nested)]
        socials: Socials,
    }

    #[derive(
        Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Deserialize, Serialize, JsonSchema,
    )]
    pub struct Socials {
        email: String,

        #[versioned(added(since = "v1beta1"))]
        mastodon: String,
    }
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

fn default_gender() -> Gender {
    Gender::Unknown
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
