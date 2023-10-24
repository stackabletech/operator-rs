use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsAuthenticationProvider {
    /// See `<https://docs.stackable.tech/home/contributor/adr/ADR016-tls-authentication.html>`.
    /// If `client_cert_secret_class` is not set, the TLS settings may also be used for client authentication.
    /// If `client_cert_secret_class` is set, the [SecretClass](https://docs.stackable.tech/secret-operator/secretclass.html)
    /// will be used to provision client certificates.
    pub client_cert_secret_class: Option<String>,
}
