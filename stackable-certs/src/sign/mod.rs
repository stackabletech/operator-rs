//! Contains primitives to create signing keys, which are used to sign CAs
//! and other certificates.
//!
//! This module currently provides the following algorithms:
//!
//! ## ECDSA
//!
//! In order to work with ECDSA keys, this crate requires two dependencies:
//! [`ecdsa`], which provides primitives and traits, and [`p256`] which
//! implements the NIST P-256 elliptic curve and supports ECDSA.
//!
//! ```
//! use stackable_certs::sign::ecdsa::SigningKey;
//! let key = SigningKey::new().unwrap();
//! ```
//!
//! ## RSA
//!
//! In order to work with RSA keys, this crate requires the [`rsa`] dependency.
//! It should be noted, that the crate is currently vulnerable to the recently
//! discovered Marvin attack. The `openssl` crate is also impacted by this. See:
//!
//! - <https://people.redhat.com/~hkario/marvin/>
//! - <https://rustsec.org/advisories/RUSTSEC-2023-0071.html>
//! - <https://github.com/RustCrypto/RSA/issues/19>
pub mod ecdsa;
pub mod rsa;

// NOTE (@Techassi): Creating a generic trait for different key algorithms is hard.
// For now, we implement the required functions directly in the impl block.
// pub trait Signer
// where
//     <Self::SigningKey as Keypair>::VerifyingKey: EncodePublicKey,
// {
//     type VerifyingKey: EncodePublicKey;
//     type SigningKey: SignatureAlgorithmIdentifier + Keypair;
//     type Error: std::error::Error + From<x509_cert::spki::Error>;

//     fn signing_key(&self) -> &Self::SigningKey;
//     fn verifying_key(&self) -> &Self::VerifyingKey;
//     fn verifying_key_pem(&self, line_ending: LineEnding) -> Result<String, Self::Error> {
//         Ok(self.verifying_key().to_public_key_pem(line_ending)?)
//     }
// }
