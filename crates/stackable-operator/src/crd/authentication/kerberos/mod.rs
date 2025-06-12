use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

#[versioned(version(name = "v1alpha1"))]
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProvider {
    /// Mandatory SecretClass used to obtain keytabs.
    pub kerberos_secret_class: String,
}
