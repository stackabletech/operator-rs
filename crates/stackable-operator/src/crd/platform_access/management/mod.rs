use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::versioned::versioned;

mod v1alpha1_impl;

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// What an agent may do with a resource inside the product beyond applying its spec.
    #[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(default, rename_all = "camelCase")]
    pub struct ManagementPolicy {
        /// Whether a resource that already exists in the product, but was not created by the
        /// agent, is taken over. `Refuse` by default.
        pub adoption: AdoptionPolicy,

        /// What happens to the resource in the product when this object is deleted. `Retain` by
        /// default.
        pub deletion: DeletionPolicy,
    }

    /// Whether an agent takes over a resource that already exists in the product.
    #[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    pub enum AdoptionPolicy {
        /// Report the existing resource as a conflict and leave it untouched.
        #[default]
        Refuse,

        /// Take over the existing resource and apply the spec to it.
        Adopt,
    }

    /// What happens to a resource in the product when the object managing it is deleted.
    #[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    pub enum DeletionPolicy {
        /// Keep the resource in the product.
        #[default]
        Retain,

        /// Delete the resource in the product, if the agent created or adopted it.
        Delete,
    }
}
