//! Contains primitives to create private keys, which are used to sign CAs
//! and bind to leaf certificates.
//!
//! This module currently provides the following algorithms:
//!
//! ## ECDSA
//!
//! In order to work with ECDSA keys, this crate requires two dependencies:
//! [`ecdsa`], which provides primitives and traits, and [`p256`] which
//! implements the NIST P-256 elliptic curve and supports ECDSA.
//!
//! ```ignore
//! use stackable_certs::keys::ecdsa::SigningKey;
//! let key = SigningKey::new().unwrap();
//! ```
//!
//! ## RSA
//!
//! In order to work with RSA keys, this crate requires the [`rsa`] dependency.
//!
//! ```ignore
//! use stackable_certs::keys::rsa::SigningKey;
//! let key = SigningKey::new().unwrap();
//! ```
//!
//! It should be noted, that the crate is currently vulnerable to the recently
//! discovered Marvin attack. The `openssl` crate is also impacted by this. See:
//!
//! - <https://people.redhat.com/~hkario/marvin/>
//! - <https://rustsec.org/advisories/RUSTSEC-2023-0071.html>
//! - <https://github.com/RustCrypto/RSA/issues/19>
use std::fmt::Debug;

use p256::pkcs8::EncodePrivateKey;
use signature::{Keypair, Signer};
use x509_cert::spki::{EncodePublicKey, SignatureAlgorithmIdentifier, SignatureBitStringEncoding};

pub mod ecdsa;
pub mod rsa;

// NOTE (@Techassi): This can _maybe_ be slightly simplified by adjusting the
// trait and using a blanket impl on types which implement Deref<Target = _>.
pub trait CertificateKeypair
where
    <Self::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
    Self: Debug + Sized,
{
    type SigningKey: SignatureAlgorithmIdentifier
        + Keypair
        + Signer<Self::Signature>
        + EncodePrivateKey;
    type Signature: SignatureBitStringEncoding;
    type VerifyingKey: EncodePublicKey;

    type Error: std::error::Error + 'static;

    /// Returns the signing (private) key half of the keypair.
    fn signing_key(&self) -> &Self::SigningKey;

    /// Returns the verifying (public) half of the keypair.
    fn verifying_key(&self) -> Self::VerifyingKey;

    /// Creates a signing key pair from the PEM-encoded private key.
    fn from_pkcs8_pem(input: &str) -> Result<Self, Self::Error>;
}
