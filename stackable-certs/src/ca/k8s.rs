use std::collections::HashSet;

use k8s_openapi::api::core::v1::Secret;
use kube::ResourceExt;
use p256::pkcs8::EncodePublicKey;
use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::{client::Client, commons::secret::SecretReference};
use x509_cert::Certificate;

use crate::{ca::CertificateAuthority, keys::KeypairExt, CertificatePair, K8sCertificatePair};

#[derive(Debug, Snafu)]
pub enum SecretError<E>
where
    E: std::error::Error + 'static,
{
    #[snafu(display("failed to retrieve secret {secret:?}"))]
    GetSecret { source: kube::Error, secret: String },

    #[snafu(display("the secret {secret:?} does not contain any data"))]
    NoSecretData { secret: String },

    #[snafu(display("the secret {secret:?} does not contain TLS certificate data"))]
    NoCertificateData { secret: String },

    #[snafu(display("the secret {secret:?} does not contain TLS private key data"))]
    NoPrivateKeyData { secret: String },

    #[snafu(display("failed to read PEM-encoded certificate chain from secret {secret:?}"))]
    ReadChain {
        source: x509_cert::der::Error,
        secret: String,
    },

    #[snafu(display("failed to parse Base64 encoded byte string"))]
    ParseBase64ByteString { source: std::str::Utf8Error },

    #[snafu(display("failed to deserialize private key from PEM"))]
    DeserializeKeyFromPem { source: E },
}

impl<S> K8sCertificatePair for CertificateAuthority<S>
where
    S: KeypairExt,
    <S::SigningKey as signature::Keypair>::VerifyingKey: EncodePublicKey,
{
    type Error = SecretError<S::Error>;

    fn from_secret(
        secret: Secret,
        key_certificate: &str,
        key_private_key: &str,
    ) -> Result<Self, Self::Error> {
        let name = secret.name_any();
        let data = secret.data.context(NoSecretDataSnafu {
            secret: name.clone(),
        })?;

        let certificate_data = data.get(key_certificate).context(NoCertificateDataSnafu {
            secret: name.clone(),
        })?;

        let certificate = Certificate::load_pem_chain(&certificate_data.0)
            .context(ReadChainSnafu {
                secret: name.clone(),
            })?
            .remove(0);

        let private_key_data = data.get(key_private_key).context(NoPrivateKeyDataSnafu {
            secret: name.clone(),
        })?;

        let private_key_data =
            std::str::from_utf8(&private_key_data.0).context(ParseBase64ByteStringSnafu)?;

        let signing_key_pair =
            S::from_pkcs8_pem(private_key_data).context(DeserializeKeyFromPemSnafu)?;

        Ok(Self {
            serial_numbers: HashSet::new(),
            certificate_pair: CertificatePair {
                key_pair: signing_key_pair,
                certificate,
            },
        })
    }

    async fn from_secret_ref(
        secret_ref: &SecretReference,
        key_certificate: &str,
        key_private_key: &str,
        client: &Client,
    ) -> Result<Self, Self::Error> {
        let secret_api = client.get_api::<Secret>(&secret_ref.namespace);
        let secret = secret_api
            .get(&secret_ref.name)
            .await
            .context(GetSecretSnafu {
                secret: secret_ref.name.clone(),
            })?;

        Self::from_secret(secret, key_certificate, key_private_key)
    }

    fn requires_renewal(&self) -> bool {
        todo!()
    }
}
