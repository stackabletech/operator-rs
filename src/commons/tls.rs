use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tls {
    /// The verification method used to verify the certificates of the server and/or the client
    pub verification: TlsVerification,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsVerification {
    /// Use TLS but don't verify certificates
    None {},
    /// Use TLS and ca certificate to verify the server
    Server(TlsServerVerification),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsServerVerification {
    /// Ca cert to verify the server
    pub ca_cert: CaCert,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaCert {
    /// Use TLS and the ca certificates trusted by the common web browsers to verify the server.
    /// This can be useful when you e.g. use public AWS S3 or other public available services.
    WebPki {},
    /// Name of the SecretClass which will provide the ca cert.
    /// Note that a SecretClass does not need to have a key but can also work with just a ca cert.
    /// So if you got provided with a ca cert but don't have access to the key you can still use this method.
    SecretClass(String),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsAuthenticationProvider {
    /// See `<https://docs.stackable.tech/home/contributor/adr/ADR016-tls-authentication.html>`.
    /// If `client_cert_secret_class` is not set, the TLS settings may also be used for client authentication.
    /// If `client_cert_secret_class` is set, the [SecretClass](https://docs.stackable.tech/secret-operator/secretclass.html)
    /// will be used to provision client certificates.
    pub client_cert_secret_class: Option<String>,
}
