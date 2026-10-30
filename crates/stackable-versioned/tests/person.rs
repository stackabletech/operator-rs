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
    let response = response_review
        .response
        .expect("conversion review must have response");

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
    #[versioned(crd(
        group = "test.stackable.tech",
        status = "PersonStatus",
        doc = "A Person resource used for testing"
    ))]
    #[derive(Clone, Debug, CustomResource, Deserialize, JsonSchema, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PersonSpec {
        username: String,

        // In v1alpha2 first and last name have been added
        #[versioned(added(in = "v1alpha2"))]
        first_name: String,

        #[versioned(added(in = "v1alpha2"))]
        last_name: String,

        // We started out with a enum. As we *need* to provide a default, we have a Unknown variant.
        // Afterwards we figured let's be more flexible and accept any arbitrary String.
        #[versioned(added(in = "v2"), changed(in = "v3", previous_type = "Gender"))]
        gender: String,

        // TODO (@Techassi): Dealing with Default here is kinda meh UX wise. What would really help
        // is enabling per-container versioning and maybe the ability to place impl blocks in here
        // for automatic implementations for all versions of a container.
        #[versioned(added(in = "v1alpha2", default = default_relationship), nested)]
        relationship: Relationship,

        #[versioned(nested)]
        socials: Socials,
    }

    // NOTE (@Techassi): This technically only needs two versions, but we cannot opt out of
    // versions (for now).
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    #[serde(rename_all = "camelCase")]
    pub enum Relationship {
        #[versioned(nested)]
        Married(Married),
        Single {},
        Other {},
    }

    // NOTE (@Techassi): This technically only needs two versions, but we cannot opt out of
    // versions (for now).
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    pub struct Married {
        with: String,

        // This field is added in the same version in which `gender` changed its type. This ensures
        // that multiple (nested) containers can track values for the same version.
        #[versioned(added(in = "v3"))]
        since: String,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    pub struct Socials {
        email: String,

        #[versioned(added(in = "v1beta1"))]
        mastodon: String,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct PersonStatus {
    pub alive: bool,
}

impl Default for PersonStatus {
    fn default() -> Self {
        Self { alive: true }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum Gender {
    #[default]
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

fn default_relationship() -> v1alpha2::Relationship {
    v1alpha2::Relationship::Other {}
}

#[cfg(test)]
impl stackable_versioned::test_utils::RoundtripTestData for v1alpha1::PersonSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        vec![
            Self {
                username: "jdoe".to_string(),
                socials: v1alpha1::Socials {
                    email: "jdoe@example.com".to_owned(),
                },
            },
            Self {
                username: String::new(),
                socials: v1alpha1::Socials {
                    email: String::new(),
                },
            },
        ]
    }
}

#[cfg(test)]
impl stackable_versioned::test_utils::RoundtripTestData for v3::PersonSpec {
    fn roundtrip_test_data() -> Vec<Self> {
        vec![
            Self {
                username: "jdoe".to_owned(),
                first_name: "John".to_owned(),
                last_name: "Doe".to_owned(),
                gender: "Unknown".to_owned(),
                relationship: v3::Relationship::Married(v3::Married {
                    with: "Jane Doe".into(),
                    since: "1970-01-01".into(),
                }),
                socials: v3::Socials {
                    email: "jdoe@example.com".to_owned(),
                    mastodon: "@jdoe@example.com".to_owned(),
                },
            },
            Self {
                username: "jdoe".to_owned(),
                first_name: "John".to_owned(),
                last_name: "Doe".to_owned(),
                gender: "Male".to_owned(),
                relationship: v3::Relationship::Single {},
                socials: v3::Socials {
                    email: "jdoe@example.com".to_owned(),
                    mastodon: "@jdoe@example.com".to_owned(),
                },
            },
            // Downgrading to v2 turns the gender into Gender::Unknown. The original value is
            // tracked in the status and restored when upgrading again.
            Self {
                username: String::new(),
                first_name: String::new(),
                last_name: String::new(),
                gender: "It's complicated".to_owned(),
                relationship: v3::Relationship::Other {},
                socials: v3::Socials {
                    email: String::new(),
                    mastodon: String::new(),
                },
            },
        ]
    }
}
