use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;

use crate::commons::{networking::HostName, tls_verification::TlsClientDetails};
#[cfg(doc)]
use crate::crd::authentication::AuthenticationClass;

mod v1alpha1_impl;

// FIXME (@Techassi): These constants should also be versioned
pub const CLIENT_ID_SECRET_KEY: &str = "clientId";
pub const CLIENT_SECRET_SECRET_KEY: &str = "clientSecret";

/// Do *not* use this for [`Url::join`], as the leading slash will erase the existing path!
const DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH: &str = "/.well-known/openid-configuration";

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// This struct contains configuration values to configure an OpenID Connect
    /// (OIDC) authentication class. Required fields are the identity provider
    /// (IdP) `hostname` and the TLS configuration. The `port` is selected
    /// automatically if not configured otherwise. The `rootPath` defaults
    /// to `/`.
    #[derive(
        Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
    )]
    #[serde(rename_all = "camelCase")]
    pub struct AuthenticationProvider {
        /// Host of the identity provider, e.g. `my.keycloak.corp` or `127.0.0.1`.
        hostname: HostName,

        /// Port of the identity provider. If TLS is used defaults to 443,
        /// otherwise to 80.
        port: Option<u16>,

        /// Root HTTP path of the identity provider. Defaults to `/`.
        #[serde(default = "v1alpha1::AuthenticationProvider::default_root_path")]
        root_path: String,

        /// Use a TLS connection. If not specified no TLS will be used.
        #[serde(flatten)]
        pub tls: TlsClientDetails,

        /// If a product extracts some sort of "effective user" that is represented by a
        /// string internally, this config determines with claim is used to extract that
        /// string. It is desirable to use `sub` in here (or some other stable identifier),
        /// but in many cases you might need to use `preferred_username` (e.g. in case of Keycloak)
        /// or a different claim instead.
        ///
        /// Please note that some products hard-coded the claim in their implementation,
        /// so some product operators might error out if the product hardcodes a different
        /// claim than configured here.
        ///
        /// We don't provide any default value, as there is no correct way of doing it
        /// that works in all setups. Most demos will probably use `preferred_username`,
        /// although `sub` being more desirable, but technically impossible with the current
        /// behavior of the products.
        pub principal_claim: String,

        /// Scopes to request from your identity provider. It is recommended to
        /// request the `openid`, `email`, and `profile` scopes.
        pub scopes: Vec<String>,

        /// This is a hint about which identity provider is used by the
        /// AuthenticationClass. Operators *can* opt to use this
        /// value to enable known quirks around OIDC / OAuth authentication.
        /// Not providing a hint means there is no hint and OIDC should be used as it is
        /// intended to be used (via the `.well-known` discovery).
        #[serde(default)]
        pub provider_hint: Option<IdentityProviderHint>,
    }

    /// An enum of supported OIDC or identity providers which can serve as a hint
    /// in the product operator. Some products require special handling of
    /// authentication related config options. This hint can be used to enable such
    /// special handling.
    #[derive(
        Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
    )]
    #[serde(rename_all = "PascalCase")]
    pub enum IdentityProviderHint {
        Keycloak,
    }

    /// OIDC specific config options. These are set on the product config level.
    #[derive(
        Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
    )]
    #[serde(rename_all = "camelCase")]
    pub struct ClientAuthenticationOptions<T = ()> {
        /// A reference to the OIDC client credentials secret. The secret contains
        /// the client id and secret.
        #[serde(rename = "clientCredentialsSecret")]
        pub client_credentials_secret_ref: String,

        /// An optional list of extra scopes which get merged with the scopes
        /// defined in the [`AuthenticationClass`].
        #[serde(default)]
        pub extra_scopes: Vec<String>,

        // If desired, operators can add custom fields that are only needed for this specific product.
        // They need to create a struct holding them and pass that as `T`.
        #[serde(flatten)]
        pub product_specific_fields: T,
    }
}
