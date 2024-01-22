use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::LocalObjectReference;
use serde::{Deserialize, Serialize};

use crate::commons::product_image_selection::PullPolicy;

/// A dynamic representation of a Helm `values.yaml` file.
///
/// This will work with any operator `values.yaml` file. It basically only
/// contains common fields which exist in all value files we use for our
/// operators.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicValues {
    pub image: ImageValues,
    pub name_override: String,
    pub full_name_override: String,
    pub service_account: ServiceAccountValues,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageValues {
    /// Specify from which repository this image should get pulled from
    pub repository: String,

    /// Specify the pull policy of this image. See more at
    /// <https://kubernetes.io/docs/concepts/containers/images/#image-pull-policy>
    pub pull_policy: PullPolicy,
    pub pull_secrets: Vec<LocalObjectReference>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceAccountValues {
    pub create: bool,
    pub annotations: BTreeMap<String, String>,
    pub name: String,
}
