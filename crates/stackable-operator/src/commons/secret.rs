use std::fmt::Display;

use k8s_openapi::api::core::v1::Secret;
use kube::runtime::reflector::ObjectRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// [`SecretReference`] represents a Kubernetes [`Secret`] reference.
///
/// In order to use this struct, the following two requirements must be met:
///
/// - Must only be used in cluster-scoped objects
/// - Namespaced objects must not be able to define cross-namespace secret
///   references
///
/// This struct is a redefinition of the one provided by k8s-openapi to make
/// name and namespace mandatory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretReference {
    /// Namespace of the Secret being referred to.
    pub namespace: String,

    /// Name of the Secret being referred to.
    pub name: String,
}

// Use ObjectRef for logging/errors
impl Display for SecretReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ObjectRef::<Secret>::from(self).fmt(f)
    }
}

impl From<SecretReference> for ObjectRef<Secret> {
    fn from(val: SecretReference) -> Self {
        ObjectRef::<Secret>::from(&val)
    }
}

impl From<&SecretReference> for ObjectRef<Secret> {
    fn from(val: &SecretReference) -> Self {
        ObjectRef::<Secret>::new(&val.name).within(&val.namespace)
    }
}
