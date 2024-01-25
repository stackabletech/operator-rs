use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::LocalObjectReference;
use serde::{Deserialize, Serialize};

use crate::{
    commons::product_image_selection::PullPolicy, cpu::CpuQuantity, memory::MemoryQuantity,
};

static EMPTY_MAP: BTreeMap<String, String> = BTreeMap::new();

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
    pub fullname_override: String,
    pub service_account: ServiceAccountValues,
    pub resources: ResourceValues,

    // TODO(Techassi): Here we could use direct Serialize and Deserialize support
    pub labels: Option<BTreeMap<String, String>>,

    /// All other keys and values.
    #[serde(flatten)]
    pub data: serde_yaml::Value,
}

impl DynamicValues {
    pub fn labels(&self) -> &BTreeMap<String, String> {
        self.labels.as_ref().unwrap_or(&EMPTY_MAP)
    }

    pub fn labels_mut(&mut self) -> &mut BTreeMap<String, String> {
        self.labels.get_or_insert_with(BTreeMap::new)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageValues {
    /// Specify from which repository this image should get pulled from.
    pub repository: String,

    /// Specify the pull policy of this image.
    ///
    /// See more at
    /// <https://kubernetes.io/docs/concepts/containers/images/#image-pull-policy>
    pub pull_policy: PullPolicy,

    /// Specify one or more pull secrets.
    ///
    /// See more at
    /// <https://kubernetes.io/docs/concepts/containers/images/#specifying-imagepullsecrets-on-a-pod>
    pub pull_secrets: Vec<LocalObjectReference>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAccountValues {
    /// Specify whether a service account should be created.
    pub create: bool,

    /// Specify which annotations to add to the service account.
    pub annotations: BTreeMap<String, String>,

    /// Specify the name of the service account.
    ///
    /// If this is not set and `create` is true, a name is generated using the
    /// fullname template.
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceValues {
    limits: ComputeResourceValues,
    requests: ComputeResourceValues,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeResourceValues {
    cpu: CpuQuantity,
    memory: MemoryQuantity,
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use rstest::rstest;

    use super::*;

    #[rstest]
    fn test_deserialize(#[files("fixtures/helm/input-*.yaml")] path: PathBuf) {
        let contents = std::fs::read_to_string(path).unwrap();
        let values: DynamicValues = serde_yaml::from_str(&contents).unwrap();
        assert_eq!(values.labels().len(), 2);
    }

    #[rstest]
    fn test_serialize(#[files("fixtures/helm/input-required.yaml")] input: PathBuf) {
        let contents = std::fs::read_to_string(input).unwrap();
        let mut values: DynamicValues = serde_yaml::from_str(&contents).unwrap();

        values
            .labels_mut()
            .insert("stackable.tech/demo".into(), "logging".into());

        let output = std::fs::read_to_string("fixtures/helm/output.yaml").unwrap();
        assert_eq!(serde_yaml::to_string(&values).unwrap(), output);
    }
}
