use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use url::{ParseError, Url};

use crate::commons::authentication::{TlsClientDetails, SECRET_BASE_PATH};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub const DEFAULT_OIDC_WELLKNOWN_PATH: &str = ".well-known/openid-configuration";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse OIDC endpoint"))]
    ParseOidcEndpoint { source: ParseError },

    #[snafu(display("failed to set OIDC endpoint scheme for endpoint {endpoint:?}"))]
    SetOidcEndpointScheme { endpoint: Url },
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
    tls: TlsClientDetails,
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
            format!("{volume_mount_path}/clientId"),
            format!("{volume_mount_path}/clientSecret"),
        )
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
            ",
        );
        assert!(oidc.is_ok());
    }

    #[test]
    fn test_oidc_http_endpoint_uri() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: my.keycloak.server
            rootPath: my-root-path
            port: 12345
            ",
        )
        .unwrap();

        assert_eq!(
            oidc.endpoint_url().unwrap().as_str(),
            "http://my.keycloak.server:12345/my-root-path"
        );
    }

    #[test]
    fn test_oidc_https_endpoint_uri() {
        let oidc = serde_yaml::from_str::<OidcAuthenticationProvider>(
            "
            hostname: my.keycloak.server
            tls:
              verification:
                server:
                  caCert:
                    secretClass: keycloak-ca-cert
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
}
