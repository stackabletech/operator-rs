use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use k8s_openapi::api::core::v1::{EnvVar, EnvVarSource, SecretKeySelector};
use snafu::{ResultExt as _, Snafu};
use url::{ParseError, Url};

use crate::{
    commons::{networking::HostName, tls_verification::TlsClientDetails},
    constants::secret::SECRET_BASE_PATH,
    crd::authentication::oidc::{
        v1alpha1::{AuthenticationProvider, IdentityProviderHint},
        CLIENT_ID_SECRET_KEY, CLIENT_SECRET_SECRET_KEY, DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH,
    },
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

// TODO (@Techassi): Move this into mod.rs
#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse OIDC endpoint url"))]
    ParseOidcEndpointUrl { source: ParseError },

    #[snafu(display(
        "failed to set OIDC endpoint scheme '{scheme}' for endpoint url \"{endpoint}\""
    ))]
    SetOidcEndpointScheme { endpoint: Url, scheme: String },
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

    /// Returns the OIDC base [`Url`] without any path segments.
    ///
    /// The base url only contains the scheme, the host, and an optional port.
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
    /// To retrieve the well-known OIDC configuration url, please use [`Self::well_known_config_url`].
    pub fn endpoint_url(&self) -> Result<Url> {
        let mut url = self.base_url()?;
        // Some tools can not cope with a trailing slash, so let's remove that
        url.set_path(self.root_path.trim_end_matches('/'));
        Ok(url)
    }

    /// Returns the well-known OIDC configuration [`Url`] without a trailing slash.
    ///
    /// The returned url is a combination of [`Self::endpoint_url`] joined with
    /// the well-known OIDC configuration path `DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH`.
    pub fn well_known_config_url(&self) -> Result<Url> {
        let mut url = self.base_url()?;

        // Taken from https://docs.rs/url/latest/url/struct.Url.html#method.join:
        // A trailing slash is significant. Without it, the last path component is considered to be
        // a “file” name to be removed to get at the “directory” that is used as the base.
        //
        // Because of that behavior, we first need to make sure that the root path doesn't contain
        // any trailing slashes to finally append the well-known config path to the url. The path
        // already contains a prefixed slash.
        let mut root_path_with_trailing_slash = self.root_path.trim_end_matches('/').to_string();
        root_path_with_trailing_slash.push_str(DEFAULT_WELLKNOWN_OIDC_CONFIG_PATH);
        url.set_path(&root_path_with_trailing_slash);

        Ok(url)
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

    pub(super) fn default_root_path() -> String {
        "/".to_string()
    }
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
            oidc.well_known_config_url().unwrap().as_str(),
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
