use kube::CustomResource;
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
