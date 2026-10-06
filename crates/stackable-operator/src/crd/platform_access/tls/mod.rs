use const_format::concatcp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    constants::secret::SECRET_BASE_PATH,
    v2::types::kubernetes::{SecretClassName, SecretName},
    versioned::versioned,
};

mod v1alpha1_impl;

/// Name of the volume holding the TLS client certificate.
pub const VOLUME_NAME: &str = "tls-client-cert";

/// Mount path for the TLS client certificate.
pub const MOUNT_PATH: &str = concatcp!(SECRET_BASE_PATH, "/", VOLUME_NAME);

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// Source of a TLS client certificate: a secret-operator SecretClass or a static Secret.
    #[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum TlsClientCredential {
        /// An AutoTLS SecretClass used to provision the certificate.
        SecretClass(SecretClassName),

        /// A static Secret holding the certificate in the keys `tls.crt` and `tls.key` (PEM), e.g. as
        /// a Secret of type `kubernetes.io/tls`.
        Secret(SecretName),
    }
}
