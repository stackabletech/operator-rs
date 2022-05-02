use serde::{Deserialize, Serialize};

use crate::commons::{ldap::LdapAuthenticationProvider, tls::TlsAuthenticationProvider};
use kube::CustomResource;
use schemars::JsonSchema;

#[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "authentication.stackable.tech",
    version = "v1alpha1",
    kind = "AuthenticationClass",
    plural = "authenticationclasses",
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationClassSpec {
    /// Provider used for authentication like LDAP or Kerberos
    pub provider: AuthenticationClassProvider,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum AuthenticationClassProvider {
    Ldap(LdapAuthenticationProvider),
    Tls(TlsAuthenticationProvider),
}
