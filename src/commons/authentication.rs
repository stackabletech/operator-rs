use serde::{Deserialize, Serialize};
use strum::Display;

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

#[derive(Clone, Debug, Deserialize, Display, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum AuthenticationClassProvider {
    Ldap(LdapAuthenticationProvider),
    Tls(TlsAuthenticationProvider),
}

#[cfg(test)]
mod test {
    use crate::commons::{
        authentication::AuthenticationClassProvider, tls::TlsAuthenticationProvider,
    };

    #[test]
    fn test_authentication_class_provider_to_string() {
        let tls_provider = AuthenticationClassProvider::Tls(TlsAuthenticationProvider {
            client_cert_secret_class: None,
        });
        assert_eq!("Tls", tls_provider.to_string())
    }
}
