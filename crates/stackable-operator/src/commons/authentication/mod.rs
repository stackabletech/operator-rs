use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, Snafu};
use strum::Display;

use crate::client::Client;

pub mod ldap;
pub mod oidc;
pub mod static_;
pub mod tls;

pub(crate) const SECRET_BASE_PATH: &str = "/stackable/secrets";

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("authentication details for OIDC were not specified. The AuthenticationClass {auth_class_name:?} uses an OIDC provider, you need to specify OIDC authentication details (such as client credentials) as well"))]
    OidcAuthenticationDetailsNotSpecified { auth_class_name: String },
}

/// The Stackable Platform uses the AuthenticationClass as a central mechanism to handle user authentication across supported products.
/// The authentication mechanism needs to be configured only in the AuthenticationClass which is then referenced in the product.
/// Multiple different authentication providers are supported.
/// Learn more in the [authentication concept documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/authentication) and the
/// [Authentication with OpenLDAP tutorial](DOCS_BASE_URL_PLACEHOLDER/tutorials/authentication_with_openldap).
#[derive(
    Clone,
    CustomResource,
    Debug,
    Deserialize,
    Eq,
    Hash,
    JsonSchema,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
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
    /// Provider used for authentication like LDAP or Kerberos.
    pub provider: AuthenticationClassProvider,
}

#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum AuthenticationClassProvider {
    /// The [static provider](https://DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_static) is used to configure a
    /// static set of users, identified by username and password.
    Static(static_::AuthenticationProvider),

    /// The [LDAP provider](DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_ldap).
    /// There is also the ["Authentication with LDAP" tutorial](DOCS_BASE_URL_PLACEHOLDER/tutorials/authentication_with_openldap)
    /// where you can learn to configure Superset and Trino with OpenLDAP.
    Ldap(ldap::AuthenticationProvider),

    /// The OIDC provider can be used to configure OpenID Connect.
    Oidc(oidc::AuthenticationProvider),

    /// The [TLS provider](DOCS_BASE_URL_PLACEHOLDER/concepts/authentication#_tls).
    /// The TLS AuthenticationClass is used when users should authenticate themselves with a TLS certificate.
    Tls(tls::AuthenticationProvider),
}

impl AuthenticationClass {
    pub async fn resolve(
        client: &Client,
        authentication_class_name: &str,
    ) -> crate::client::Result<AuthenticationClass> {
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
/// Additionally, it might be the case that special fields are needed in the
/// contained structs, such as [`oidc::ClientAuthenticationOptions`]. To be able
/// to add custom fields in that structs without serde(flattening) multiple structs,
/// they are generic, so you can add additional attributes if needed.
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
#[schemars(description = "")]
pub struct ClientAuthenticationDetails<O = ()> {
    /// Name of the [AuthenticationClass](https://docs.stackable.tech/home/nightly/concepts/authentication) used to
    /// authenticate users.
    //
    // To get the concrete [`AuthenticationClass`], we must resolve it. This resolution can be achieved by using
    // [`ClientAuthenticationDetails::resolve_class`].
    #[serde(rename = "authenticationClass")]
    authentication_class_ref: String,

    /// This field contains OIDC-specific configuration. It is only required in case OIDC is used.
    //
    // Use [`ClientAuthenticationDetails::oidc_or_error`] to get the value or report an error to the user.
    // TODO: Ideally we want this to be an enum once other `ClientAuthenticationOptions` are added, so
    // that user can not configure multiple options at the same time (yes we are aware that this makes a
    // changing the type of an AuthenticationClass harder).
    // This is a non-breaking change though :)
    oidc: Option<oidc::ClientAuthenticationOptions<O>>,
}

impl<O> ClientAuthenticationDetails<O> {
    /// Resolves this specific [`AuthenticationClass`]. Usually products support
    /// a list of authentication classes, which individually need to be resolved.crate::client
    pub async fn resolve_class(
        &self,
        client: &Client,
    ) -> crate::client::Result<AuthenticationClass> {
        AuthenticationClass::resolve(client, &self.authentication_class_ref).await
    }

    pub fn authentication_class_name(&self) -> &String {
        &self.authentication_class_ref
    }

    /// In case OIDC is configured, the user *needs* to provide some connection details,
    /// such as the client credentials. Call this function in case the user has configured
    /// OIDC, as it will error out then the OIDC client details are missing.
    pub fn oidc_or_error(
        &self,
        auth_class_name: &str,
    ) -> Result<&oidc::ClientAuthenticationOptions<O>> {
        self.oidc
            .as_ref()
            .with_context(|| OidcAuthenticationDetailsNotSpecifiedSnafu {
                auth_class_name: auth_class_name.to_string(),
            })
    }
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
