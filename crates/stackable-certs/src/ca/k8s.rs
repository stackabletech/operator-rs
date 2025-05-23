use k8s_openapi::api::core::v1::Secret;
use kube::runtime::reflector::ObjectRef;
use rsa::pkcs8::EncodePublicKey;
use snafu::{OptionExt, ResultExt, Snafu, ensure};
use stackable_operator::{client::Client, commons::secret::SecretReference};
use tracing::{debug, instrument};

use super::CertificateAuthority;
use crate::{CertificatePair, keys::CertificateKeypair};

pub const TLS_SECRET_TYPE: &str = "kubernetes.io/tls";

/// Defines all error variants which can occur when loading a CA from a Kubernetes [`Secret`].
#[derive(Debug, Snafu)]
pub enum SecretError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("failed to retrieve secret \"{secret_ref}\""))]
    GetSecret {
        source: kube::Error,
        secret_ref: SecretReference,
    },

    #[snafu(display("invalid secret type, expected {TLS_SECRET_TYPE}"))]
    InvalidSecretType,

    #[snafu(display("the secret {secret:?} does not contain any data"))]
    NoSecretData { secret: ObjectRef<Secret> },

    #[snafu(display("the secret {secret:?} does not contain TLS certificate data"))]
    NoCertificateData { secret: ObjectRef<Secret> },

    #[snafu(display("the secret {secret:?} does not contain TLS private key data"))]
    NoPrivateKeyData { secret: ObjectRef<Secret> },

    #[snafu(display("failed to read PEM-encoded certificate chain from secret {secret:?}"))]
    ReadChain {
        source: x509_cert::der::Error,
        secret: ObjectRef<Secret>,
    },

    #[snafu(display("failed to parse UTF-8 encoded byte string"))]
    DecodeUtf8String { source: std::str::Utf8Error },

    #[snafu(display("failed to deserialize private key from PEM"))]
    DeserializeKeyFromPem { source: E },
}

/// Create a [`CertificateAuthority`] from a Kubernetes [`Secret`].
///
/// Both the `certificate_key` and `private_key_key` parameters describe
/// the _key_ used to lookup the certificate and private key value in the
/// Kubernetes [`Secret`]. Common keys are `ca.crt` and `ca.key`.
#[instrument(skip(secret))]
pub fn ca_from_k8s_secret<SK>(
    secret: Secret,
    certificate_key: &str,
    private_key_key: &str,
) -> Result<CertificateAuthority<SK>, SecretError<SK::Error>>
where
    SK: CertificateKeypair,
    <SK::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    ensure!(
        secret.type_.as_ref().is_none_or(|s| s != TLS_SECRET_TYPE),
        InvalidSecretTypeSnafu
    );
    let data = secret.data.as_ref().with_context(|| NoSecretDataSnafu {
        secret: ObjectRef::from_obj(&secret),
    })?;

    debug!("retrieving certificate data from secret via key \"{certificate_key}\"");
    let certificate_data = data
        .get(certificate_key)
        .with_context(|| NoCertificateDataSnafu {
            secret: ObjectRef::from_obj(&secret),
        })?;

    let certificate = x509_cert::Certificate::load_pem_chain(&certificate_data.0)
        .with_context(|_| ReadChainSnafu {
            secret: ObjectRef::from_obj(&secret),
        })?
        .remove(0);

    debug!("retrieving private key data from secret via key \"{private_key_key}\"");
    let private_key_data = data
        .get(private_key_key)
        .with_context(|| NoPrivateKeyDataSnafu {
            secret: ObjectRef::from_obj(&secret),
        })?;

    let private_key_data =
        std::str::from_utf8(&private_key_data.0).context(DecodeUtf8StringSnafu)?;

    let signing_key_pair =
        SK::from_pkcs8_pem(private_key_data).context(DeserializeKeyFromPemSnafu)?;

    Ok(CertificateAuthority::new(CertificatePair::new(
        certificate,
        signing_key_pair,
    )))
}

/// Create a [`CertificateAuthority`] from a Kubernetes [`SecretReference`].
#[instrument(skip(secret_ref, client))]
pub async fn ca_from_k8s_secret_ref<SK>(
    secret_ref: &SecretReference,
    certificate_key: &str,
    private_key_key: &str,
    client: &Client,
) -> Result<CertificateAuthority<SK>, SecretError<SK::Error>>
where
    SK: CertificateKeypair,
    <SK::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    let secret_api = client.get_api::<Secret>(&secret_ref.namespace);
    let secret = secret_api
        .get(&secret_ref.name)
        .await
        .with_context(|_| GetSecretSnafu {
            secret_ref: secret_ref.to_owned(),
        })?;

    ca_from_k8s_secret(secret, certificate_key, private_key_key)
}
