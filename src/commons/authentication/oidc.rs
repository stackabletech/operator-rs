use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use url::{ParseError, Url};

use crate::commons::authentication::{
    oidc::oidc_error::ParseOidcEndpointSnafu, TlsClientUsage, SECRET_BASE_PATH,
};

use self::oidc_error::SetOidcEndpointSchemeSnafu;

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum OidcError {
    #[snafu(display("failed to parse OIDC endpoint"))]
    ParseOidcEndpoint { source: ParseError },

    #[snafu(display("failed to set OIDC endpoint scheme for endpoint {endpoint:?}"))]
    SetOidcEndpointScheme { endpoint: Url },
}

pub type Result<T> = std::result::Result<T, OidcError>;

/// TODO: docs
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcAuthenticationProvider {
    /// Hostname of the identity provider
    pub hostname: String,

    /// Port of the identity provider. If TLS is used defaults to 443, otherwise to 80
    pub port: Option<u16>,

    /// Root HTTP path of the identity provider. Defaults to `/`.
    #[serde(default = "default_root_path")]
    pub root_path: String,

    /// Use a TLS connection. If not specified no TLS will be used
    #[serde(flatten)]
    pub tls: TlsClientUsage,
}

fn default_root_path() -> String {
    "/".to_string()
}

impl OidcAuthenticationProvider {
    pub fn endpoint_uri(&self) -> Result<Url> {
        let mut url = Url::parse(&format!("{}:{}", self.hostname, self.port()))
            .context(ParseOidcEndpointSnafu)?;
        url.set_scheme(if self.tls.use_tls() { "https" } else { "http" })
            .context(SetOidcEndpointSchemeSnafu { endpoint: url })?;
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
            port: 389
            rootPath: /
            ",
        );
        assert!(oidc.is_ok());
    }
}
