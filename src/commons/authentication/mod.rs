pub mod ldap;
pub mod oidc;
pub mod static_;
pub mod tls;

use crate::{
    builder::{ContainerBuilder, PodBuilder, VolumeMountBuilder},
    commons::secret_class::SecretClassVolume,
};

use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::Display;

pub use crate::{client::Client, error::Error};

const SECRET_BASE_PATH: &str = "/stackable/secrets";

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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsClientDetails {
    /// Use a TLS connection. If not specified no TLS will be used
    pub tls: Option<Tls>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tls {
    /// The verification method used to verify the certificates of the server and/or the client
    pub verification: TlsVerification,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsVerification {
    /// Use TLS but don't verify certificates
    None {},

    /// Use TLS and ca certificate to verify the server
    Server(TlsServerVerification),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsServerVerification {
    /// Ca cert to verify the server
    pub ca_cert: CaCert,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaCert {
    /// Use TLS and the ca certificates trusted by the common web browsers to verify the server.
    /// This can be useful when you e.g. use public AWS S3 or other public available services.
    WebPki {},

    /// Name of the SecretClass which will provide the ca cert.
    /// Note that a SecretClass does not need to have a key but can also work with just a ca cert.
    /// So if you got provided with a ca cert but don't have access to the key you can still use this method.
    SecretClass(String),
}

impl TlsClientDetails {
    /// This functions adds
    ///
    /// * The needed volumes to the PodBuilder
    /// * The needed volume_mounts to all the ContainerBuilder in the list (e.g. init + main container)
    ///
    /// This function will handle
    ///
    /// * Tls secret class used to verify the cert of the LDAP server
    pub fn add_volumes_and_mounts(
        &self,
        pod_builder: &mut PodBuilder,
        container_builders: Vec<&mut ContainerBuilder>,
    ) {
        let (volumes, mounts) = self.volumes_and_mounts();
        pod_builder.add_volumes(volumes);
        for cb in container_builders {
            cb.add_volume_mounts(mounts.clone());
        }
    }

    /// It is recommended to use [`Self::add_volumes_and_mounts`], this function returns you the
    /// volumes and mounts in case you need to add them by yourself.
    pub fn volumes_and_mounts(&self) -> (Vec<Volume>, Vec<VolumeMount>) {
        let mut volumes = Vec::new();
        let mut mounts = Vec::new();

        if let Some(secret_class) = self.tls_ca_cert_secret_class() {
            let volume_name = format!("{secret_class}-ca-cert");
            volumes.push(
                SecretClassVolume {
                    secret_class: secret_class.to_string(),
                    scope: None,
                }
                .to_volume(&volume_name),
            );
            mounts.push(
                VolumeMountBuilder::new(volume_name, format!("{SECRET_BASE_PATH}/{secret_class}"))
                    .build(),
            );
        }

        (volumes, mounts)
    }

    /// Whether TLS is configured
    pub const fn use_tls(&self) -> bool {
        self.tls.is_some()
    }

    /// Whether TLS verification is configured. Returns false if TLS itself isn't configured
    pub fn use_tls_verification(&self) -> bool {
        self.tls
            .as_ref()
            .map(|tls| tls.verification != TlsVerification::None {})
            .unwrap_or_default()
    }

    /// Returns the path of the ca.crt that should be used to verify the LDAP server certificate
    /// if TLS verification with a CA cert from a SecretClass is configured.
    pub fn tls_ca_cert_mount_path(&self) -> Option<String> {
        self.tls_ca_cert_secret_class()
            .map(|secret_class| format!("{SECRET_BASE_PATH}/{secret_class}/ca.crt"))
    }

    /// Extracts the SecretClass that provides the CA cert used to verify the server certificate.
    fn tls_ca_cert_secret_class(&self) -> Option<String> {
        if let Some(Tls {
            verification:
                TlsVerification::Server(TlsServerVerification {
                    ca_cert: CaCert::SecretClass(secret_class),
                }),
        }) = &self.tls
        {
            Some(secret_class.to_owned())
        } else {
            None
        }
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
    pub fn resolve_class(&self) -> Result<AuthenticationClass, Error> {
        todo!()
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
    Ldap(ldap::ClientAuthenticationOptions),
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
