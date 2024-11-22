use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use k8s_openapi::api::core::v1::{EnvVar, EnvVarSource, SecretKeySelector};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use url::{ParseError, Url};

#[cfg(doc)]
use crate::commons::authentication::AuthenticationClass;
use crate::commons::{
    authentication::SECRET_BASE_PATH, networking::HostName, tls_verification::TlsClientDetails,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub const CLIENT_ID_SECRET_KEY: &str = "clientId";
pub const CLIENT_SECRET_SECRET_KEY: &str = "clientSecret";

const DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH: &str = ".well-known/openid-configuration";

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse OIDC endpoint url"))]
    ParseOidcEndpointUrl { source: ParseError },

    #[snafu(display(
        "failed to set OIDC endpoint scheme '{scheme}' for endpoint url \"{endpoint}\""
    ))]
    SetOidcEndpointScheme { endpoint: Url, scheme: String },

    #[snafu(display("failed to join the path {path:?} to URL \"{url}\""))]
    JoinPath {
        source: ParseError,
        url: Url,
        path: String,
    },
}

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
    #[serde(default = "default_root_path")]
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

fn default_root_path() -> String {
    "/".to_string()
}

impl AuthenticationProvider {
    pub fn new(
        hostname: HostName,
        port: Option<u16>,
        root_path: String,
        tls: TlsClientDetails,
        principal_claim: String,
        scopes: Vec<String>,
        provider_hint: Option<IdentityProviderHint>,
    ) -> Self {
        Self {
            hostname,
            port,
            root_path,
            tls,
            principal_claim,
            scopes,
            provider_hint,
        }
    }

    /// Base [`Url`] without any path set (so only protocol, host, port and such).
    fn base_url(&self) -> Result<Url> {
        let mut url = Url::parse(&format!(
            "http://{host}:{port}",
            host = self.hostname.as_url_host(),
            port = self.port()
        ))
        .context(ParseOidcEndpointUrlSnafu)?;

        if self.tls.uses_tls() {
            url.set_scheme("https").map_err(|_| {
                SetOidcEndpointSchemeSnafu {
                    scheme: "https".to_string(),
                    endpoint: url.clone(),
                }
                .build()
            })?;
        }

        Ok(url)
    }

    /// Returns the OIDC endpoint [`Url`] without a trailing slash.
    ///
    /// To get the well-known URL, please use [`Self::well_known_url`].
    pub fn endpoint_url(&self) -> Result<Url> {
        let mut url = self.base_url()?;
        // Some tools can not cope with a trailing slash, so let's remove that
        url.set_path(self.root_path.trim_end_matches('/'));
        Ok(url)
    }

    /// Returns the well-known [`Url`] without a trailing slash.
    ///
    /// It is basically the [`Self::endpoint_url`] joined with
    /// "./.well-known/openid-configuration", while watching out for URL joining madness.
    pub fn well_known_url(&self) -> Result<Url> {
        let mut url = self.base_url()?;

        // Url::join cuts of the part after the last slash :/
        // So we need to make sure we have a trailing slash, so that nothing get's cut of.
        let mut root_path_with_trailing_slash = self.root_path.trim_end_matches('/').to_string();
        root_path_with_trailing_slash.push('/');
        url.set_path(&root_path_with_trailing_slash);
        url.join(DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH)
            .with_context(|_| JoinPathSnafu {
                url: url.clone(),
                path: DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH.to_owned(),
            })
    }

    /// Returns the port to be used, which is either user configured or defaulted based upon TLS usage
    pub fn port(&self) -> u16 {
        self.port
            .unwrap_or(if self.tls.uses_tls() { 443 } else { 80 })
    }

    /// Returns the path of the files containing client id and secret in case they are given.
    pub fn client_credentials_volume_mount_path(secret_name: &str) -> String {
        // This mount path can not clash, as Secret names are unique within a Namespace.
        format!("{SECRET_BASE_PATH}/{secret_name}")
    }

    /// Returns the path of the files containing client id and secret in case they are given.
    pub fn client_credentials_mount_paths(secret_name: &str) -> (String, String) {
        let volume_mount_path = Self::client_credentials_volume_mount_path(secret_name);

        (
            format!("{volume_mount_path}/{CLIENT_ID_SECRET_KEY}"),
            format!("{volume_mount_path}/{CLIENT_SECRET_SECRET_KEY}"),
        )
    }

    /// Name of the clientId and clientSecret env variables.
    ///
    /// Env variables need to be C_IDENTIFIER according to k8s docs. We could replace `-` with `_` and `.` with `_`,
    /// but this could cause collisions. To be collision-free we hash the secret key instead and use the hash in the env var.
    pub fn client_credentials_env_names(secret_name: &str) -> (String, String) {
        let mut hasher = DefaultHasher::new();
        secret_name.hash(&mut hasher);
        let secret_name_hash = hasher.finish();

        // Prefix with zeros to have consistent length. Max length is 16 characters, which is caused by [`u64::MAX`].
        let secret_name_hash = format!("{:016x}", secret_name_hash).to_uppercase();
        let env_var_prefix = format!("OIDC_{secret_name_hash}");

        (
            format!("{env_var_prefix}_CLIENT_ID"),
            format!("{env_var_prefix}_CLIENT_SECRET"),
        )
    }

    pub fn client_credentials_env_var_mounts(secret_name: String) -> Vec<EnvVar> {
        let (client_id_env_var, client_secret_env_var) =
            Self::client_credentials_env_names(&secret_name);

        vec![
            EnvVar {
                name: client_id_env_var,
                value_from: Some(EnvVarSource {
                    secret_key_ref: Some(SecretKeySelector {
                        key: CLIENT_ID_SECRET_KEY.to_string(),
                        name: secret_name.clone(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            EnvVar {
                name: client_secret_env_var,
                value_from: Some(EnvVarSource {
                    secret_key_ref: Some(SecretKeySelector {
                        key: CLIENT_SECRET_SECRET_KEY.to_string(),
                        name: secret_name,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ]
    }
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

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::*;

    #[test]
    fn minimal() {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: my.keycloak.server
            scopes: [openid]
            principalClaim: preferred_username
            ",
        );
        assert!(oidc.is_ok());
    }

    #[test]
    fn full() {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: my.keycloak.server
            rootPath: /
            port: 12345
            scopes: [openid]
            principalClaim: preferred_username
            ",
        );
        assert!(oidc.is_ok());
    }

    #[test]
    fn http_endpoint_url() {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: my.keycloak.server
            rootPath: my-root-path
            port: 12345
            scopes: [openid]
            principalClaim: preferred_username
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url().unwrap().as_str(),
            "http://my.keycloak.server:12345/my-root-path"
        );
    }

    #[test]
    fn https_endpoint_url() {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: my.keycloak.server
            tls:
              verification:
                server:
                  caCert:
                    secretClass: keycloak-ca-cert
            scopes: [openid]
            principalClaim: preferred_username
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url().unwrap().as_str(),
            "https://my.keycloak.server/"
        );
    }

    #[test]
    fn ipv6_endpoint_url() {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(
            "
            hostname: 2606:2800:220:1:248:1893:25c8:1946
            rootPath: my-root-path
            port: 12345
            scopes: [openid]
            principalClaim: preferred_username
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url().unwrap().as_str(),
            "http://[2606:2800:220:1:248:1893:25c8:1946]:12345/my-root-path"
        );
    }

    #[rstest]
    #[case("/", "http://my.keycloak.server:1234/")]
    #[case("/realms/sdp", "http://my.keycloak.server:1234/realms/sdp")]
    #[case("/realms/sdp/", "http://my.keycloak.server:1234/realms/sdp")]
    #[case("/realms/sdp//////", "http://my.keycloak.server:1234/realms/sdp")]
    #[case(
        "/realms/my/realm/with/slashes//////",
        "http://my.keycloak.server:1234/realms/my/realm/with/slashes"
    )]
    fn root_path_endpoint_url(#[case] root_path: String, #[case] expected_endpoint_url: &str) {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(&format!(
            "
            hostname: my.keycloak.server
            port: 1234
            rootPath: {root_path}
            scopes: [openid]
            principalClaim: preferred_username
            "
        ))
        .unwrap();

        assert_eq!(oidc.endpoint_url().unwrap().as_str(), expected_endpoint_url);
    }

    #[rstest]
    #[case("/", "https://my.keycloak.server/.well-known/openid-configuration")]
    #[case(
        "/realms/sdp",
        "https://my.keycloak.server/realms/sdp/.well-known/openid-configuration"
    )]
    #[case(
        "/realms/sdp/",
        "https://my.keycloak.server/realms/sdp/.well-known/openid-configuration"
    )]
    #[case(
        "/realms/sdp//////",
        "https://my.keycloak.server/realms/sdp/.well-known/openid-configuration"
    )]
    #[case(
        "/realms/my/realm/with/slashes//////",
        "https://my.keycloak.server/realms/my/realm/with/slashes/.well-known/openid-configuration"
    )]
    fn root_path_well_known_url(#[case] root_path: String, #[case] expected_well_known_url: &str) {
        let oidc = serde_yaml::from_str::<AuthenticationProvider>(&format!(
            "
            hostname: my.keycloak.server
            rootPath: {root_path}
            scopes: [openid]
            principalClaim: preferred_username
            tls:
              verification:
                server:
                  caCert:
                    webPki: {{}}
            "
        ))
        .unwrap();

        assert_eq!(
            oidc.well_known_url().unwrap().as_str(),
            expected_well_known_url
        );
    }

    #[test]
    fn client_env_vars() {
        let secret_name = "my-keycloak-client";
        let env_names = AuthenticationProvider::client_credentials_env_names(secret_name);
        assert_eq!(
            env_names,
            (
                "OIDC_68098419C6E0D0C6_CLIENT_ID".to_string(),
                "OIDC_68098419C6E0D0C6_CLIENT_SECRET".to_string()
            )
        );
        let env_var_mounts =
            AuthenticationProvider::client_credentials_env_var_mounts(secret_name.to_string());
        assert_eq!(
            env_var_mounts
                .iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>(),
            vec![env_names.0, env_names.1],
        );
        assert_eq!(
            env_var_mounts
                .iter()
                .map(|e| e.value_from.clone().unwrap().secret_key_ref.unwrap())
                .collect::<Vec<_>>(),
            vec![
                SecretKeySelector {
                    key: CLIENT_ID_SECRET_KEY.to_string(),
                    name: secret_name.to_string(),
                    optional: None,
                },
                SecretKeySelector {
                    key: CLIENT_SECRET_SECRET_KEY.to_string(),
                    name: secret_name.to_string(),
                    optional: None,
                }
            ],
        );
    }
}
