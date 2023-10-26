use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use k8s_openapi::api::core::v1::{EnvVar, EnvVarSource, SecretKeySelector};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use url::{ParseError, Url};

use crate::commons::authentication::{TlsClientDetails, SECRET_BASE_PATH};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub const DEFAULT_OIDC_WELLKNOWN_PATH: &str = ".well-known/openid-configuration";
pub const CLIENT_ID_SECRET_KEY: &str = "clientId";
pub const CLIENT_SECRET_SECRET_KEY: &str = "clientSecret";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse OIDC endpoint"))]
    ParseOidcEndpoint { source: ParseError },

    #[snafu(display("failed to set OIDC endpoint scheme '{scheme}' for endpoint '{endpoint}'"))]
    SetOidcEndpointScheme { endpoint: Url, scheme: String },
}

/// This struct contains configuration values to configure an OpenID Connect
/// (OIDC) authentication class. Required fields are the identity provider
/// (IdP) `hostname` and the TLS configuration. The `port` is selected
/// automatically if not configured otherwise. The `rootPath` defaults
/// to `/`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcAuthenticationProvider {
    /// Hostname of the identity provider
    hostname: String,

    /// Port of the identity provider. If TLS is used defaults to 443, otherwise to 80
    port: Option<u16>,

    /// Root HTTP path of the identity provider. Defaults to `/`.
    #[serde(default = "default_root_path")]
    root_path: String,

    /// Use a TLS connection. If not specified no TLS will be used
    #[serde(flatten)]
    pub tls: TlsClientDetails,

    /// Scopes to request from your Identity Provider.
    /// E.g. for keycloak you need to at least request the `openid` scope.
    pub scopes: Vec<String>,
}

fn default_root_path() -> String {
    "/".to_string()
}

impl OidcAuthenticationProvider {
    /// Returns the OIDC endpoint [`Url`]. To append the default OIDC well-known
    /// configuration path, use `url.join()`. This module provides the default
    /// path at [`DEFAULT_OIDC_WELLKNOWN_PATH`].
    pub fn endpoint_url(&self) -> Result<Url> {
        let mut url = Url::parse(&format!("http://{}:{}", self.hostname, self.port()))
            .context(ParseOidcEndpointSnafu)?;

        if self.tls.use_tls() {
            url.set_scheme("https").map_err(|_| {
                SetOidcEndpointSchemeSnafu {
                    scheme: "https".to_string(),
                    endpoint: url.clone(),
                }
                .build()
            })?;
        }

        url.set_path(&self.root_path);
        Ok(url)
    }

    /// Returns the port to be used, which is either user configured or defaulted based upon TLS usage
    pub fn port(&self) -> u16 {
        self.port
            .unwrap_or(if self.tls.use_tls() { 443 } else { 80 })
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
        // Prefix with zeros to have consistent length. Max length is 16 characters, which corresponds to u64::MAX
        let secret_name_hash = format!("{:016}", secret_name_hash).to_uppercase();
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
                        name: Some(secret_name.clone()),
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
                        name: Some(secret_name),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_oidc_minimal() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: my.keycloak.server
            scopes: [openid]
            ",
        );
        assert!(oidc.is_ok());
    }

    #[test]
    fn test_oidc_full() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: my.keycloak.server
            rootPath: /
            port: 12345
            scopes: [openid]
            ",
        );
        assert!(oidc.is_ok());
    }

    #[test]
    fn test_oidc_http_endpoint_url() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: my.keycloak.server
            rootPath: my-root-path
            port: 12345
            scopes: [openid]
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url().unwrap().as_str(),
            "http://my.keycloak.server:12345/my-root-path"
        );
    }

    #[test]
    fn test_oidc_https_endpoint_url() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: my.keycloak.server
            tls:
              verification:
                server:
                  caCert:
                    secretClass: keycloak-ca-cert
            scopes: [openid]
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url()
                .unwrap()
                .join(DEFAULT_OIDC_WELLKNOWN_PATH)
                .unwrap()
                .as_str(),
            "https://my.keycloak.server/.well-known/openid-configuration"
        );
    }

    #[test]
    fn test_oidc_ipv6_endpoint_url() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: '[2606:2800:220:1:248:1893:25c8:1946]'
            rootPath: my-root-path
            port: 12345
            scopes: [openid]
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url().unwrap().as_str(),
            "http://[2606:2800:220:1:248:1893:25c8:1946]:12345/my-root-path"
        );
    }

    #[test]
    fn test_oidc_client_env_vars() {
        let secret_name = "my-keycloak-client";
        let env_names = OidcAuthenticationProvider::client_credentials_env_names(secret_name);
        assert_eq!(
            env_names,
            (
                "OIDC_7496668300980572358_CLIENT_ID".to_string(),
                "OIDC_7496668300980572358_CLIENT_SECRET".to_string()
            )
        );
        let env_var_mounts =
            OidcAuthenticationProvider::client_credentials_env_var_mounts(secret_name.to_string());
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
                    name: Some(secret_name.to_string()),
                    optional: None,
                },
                SecretKeySelector {
                    key: CLIENT_SECRET_SECRET_KEY.to_string(),
                    name: Some(secret_name.to_string()),
                    optional: None,
                }
            ],
        );
    }
}
