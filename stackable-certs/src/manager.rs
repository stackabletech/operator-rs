use p256::pkcs8::EncodePublicKey;
use signature::Keypair;

use crate::{ca::CertificateAuthority, keys::KeypairExt};

// NOTE (@Techassi): We make the manager generic over the signing key the inner
// CAs use. It should be noted, that we then can only support one kind of
// signing key at once. We cannot mix RSA and ECDSA keys using the same manager.
pub struct Manager<S>
where
    S: KeypairExt,
    <S::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
{
    certificate_authorities: Vec<CertificateAuthority<S>>,
}

impl<S> Manager<S>
where
    S: KeypairExt,
    <S::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
{
    pub fn new() -> Self {
        Self {
            certificate_authorities: Vec::new(),
        }
    }
}
