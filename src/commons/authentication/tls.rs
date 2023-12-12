use k8s_openapi::api::core::v1::{Volume, VolumeMount};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    builder::{ContainerBuilder, PodBuilder, VolumeMountBuilder},
    commons::{authentication::SECRET_BASE_PATH, secret_class::SecretClassVolume},
};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProvider {
    /// See [ADR017: TLS authentication](DOCS_BASE_URL_PLACEHOLDER/contributor/adr/adr017-tls_authentication).
    /// If `client_cert_secret_class` is not set, the TLS settings may also be used for client authentication.
    /// If `client_cert_secret_class` is set, the [SecretClass](DOCS_BASE_URL_PLACEHOLDER/secret-operator/secretclass)
    /// will be used to provision client certificates.
    pub client_cert_secret_class: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsClientDetails {
    /// Use a TLS connection. If not specified no TLS will be used.
    pub tls: Option<Tls>,
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
    pub const fn uses_tls(&self) -> bool {
        self.tls.is_some()
    }

    /// Whether TLS verification is configured. Returns `false` if TLS itself isn't configured
    pub fn uses_tls_verification(&self) -> bool {
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
    pub(crate) fn tls_ca_cert_secret_class(&self) -> Option<String> {
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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tls {
    /// The verification method used to verify the certificates of the server and/or the client.
    pub verification: TlsVerification,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsVerification {
    /// Use TLS but don't verify certificates.
    None {},

    /// Use TLS and a CA certificate to verify the server.
    Server(TlsServerVerification),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsServerVerification {
    /// CA cert to verify the server.
    pub ca_cert: CaCert,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaCert {
    /// Use TLS and the CA certificates trusted by the common web browsers to verify the server.
    /// This can be useful when you e.g. use public AWS S3 or other public available services.
    WebPki {},

    /// Name of the [SecretClass](DOCS_BASE_URL_PLACEHOLDER/secret-operator/secretclass) which will provide the CA certificate.
    /// Note that a SecretClass does not need to have a key but can also work with just a CA certificate,
    /// so if you got provided with a CA cert but don't have access to the key you can still use this method.
    SecretClass(String),
}
