use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::Display;

pub use crate::{client::Client, error::Error};

pub mod ldap;
pub mod oidc;
pub mod static_;
pub mod tls;

pub(crate) const SECRET_BASE_PATH: &str = "/stackable/secrets";

#[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
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
    Static(static_::AuthenticationProvider),
    Ldap(ldap::AuthenticationProvider),
    Oidc(oidc::AuthenticationProvider),
    Tls(tls::AuthenticationProvider),
}

impl AuthenticationClass {
    pub async fn resolve(
        client: &Client,
        authentication_class_name: &str,
    ) -> Result<AuthenticationClass, Error> {
        client
            .get::<AuthenticationClass>(authentication_class_name, &()) // AuthenticationClass has ClusterScope
            .await
    }
}

/// Common [`ClientAuthenticationDetails`] which is specified at the client/
/// product cluster level. It provides a name (key) to resolve a particular
/// [`AuthenticationClass`]. Additionally, it provides authentication provider
/// specific configuration (OIDC and LDAP for example).
///
/// If the product needs additional (product specific) authentication options,
/// it is recommended to wrap this struct and use `#[serde(flatten)]` on the
/// field.
///
/// ### Example
///
/// ```
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// use stackable_operator::commons::authentication::ClientAuthenticationDetails;
///
/// #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
/// #[serde(rename_all = "camelCase")]
/// pub struct SupersetAuthenticationClass {
///     pub user_registration_role: String,
///     pub user_registration: bool,
///
///     #[serde(flatten)]
///     pub common: ClientAuthenticationDetails,
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientAuthenticationDetails {
    /// A name/key which references an authentication class. To get the concrete
    /// [`AuthenticationClass`], we must resolve it. This resolution can be
    /// achieved by using [`ClientAuthenticationDetails::resolve_class`].
    #[serde(rename = "authenticationClass")]
    authentication_class_ref: String,

    /// This field contains authentication provider specific configuration. It
    /// is flattened into the final CRD.
    #[serde(flatten)]
    config: ClientAuthenticationConfig,
}

impl ClientAuthenticationDetails {
    /// Resolves this specific [`AuthenticationClass`]. Usually products support
    /// a list of authentication classes, which indivually need to be resolved.
    pub async fn resolve_class(&self, client: &Client) -> Result<AuthenticationClass, Error> {
        AuthenticationClass::resolve(client, &self.authentication_class_ref).await
    }
}

/// An enum of supported authentication providers. Each variant contains
/// provider specific options. The structure is based on disussions around the
/// [OIDC ADR][oidc-adr].
///
/// [oidc-adr]: https://docs.stackable.tech/home/nightly/contributor/adr/adr032-oauth-oidc-support
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientAuthenticationConfig {
    Oidc(oidc::ClientAuthenticationOptions),
    // NOTE (Techassi): Commented out for now, as LDAP uses the old syntax
    // Ldap(ldap::ClientAuthenticationOptions),
}

#[cfg(test)]
mod test {
    use crate::commons::authentication::{
        tls::AuthenticationProvider, AuthenticationClassProvider,
    };

    #[test]
    fn test_authentication_class_provider_to_string() {
        let tls_provider = AuthenticationClassProvider::Tls(AuthenticationProvider {
            client_cert_secret_class: None,
        });
        assert_eq!("Tls", tls_provider.to_string())
    }
}
