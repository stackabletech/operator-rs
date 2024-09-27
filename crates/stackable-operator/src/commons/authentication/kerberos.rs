use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProvider {
    /// Mandatory SecretClass used to obtain keytabs.
    #[serde(default)]
    pub kerberos_secret_class: String,
}
