use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

#[versioned(version(name = "v1alpha1"))]
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProvider {
    /// See [ADR017: TLS authentication](DOCS_BASE_URL_PLACEHOLDER/contributor/adr/adr017-tls_authentication).
    /// If `client_cert_secret_class` is not set, the TLS settings may also be used for client authentication.
    /// If `client_cert_secret_class` is set, the [SecretClass](DOCS_BASE_URL_PLACEHOLDER/secret-operator/secretclass)
    /// will be used to provision client certificates.
    pub client_cert_secret_class: Option<String>,
}
