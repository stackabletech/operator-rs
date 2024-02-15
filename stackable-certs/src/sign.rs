//! Contains primitives to create signing keys, which are used to sign CAs
//! and other certificates.
//!
//! This module currently provides the following signature algorithms:
//!
//! ## ED25519
//!
//! The ED25519 implemention is provided by the [`ed25519_dalek`] crate, which
//! internally makes use of signature primitives provided by the `ed25519`
//! crate.
//!
//! Cryptographically secure pseudo-randum number generation is provided by the
//! [`rand`] crate. The [`ed25519_dalek`] requires the `rand_core` feature to be
//! enabled to use the [`SigningKey::generate()`] function.
//!
//! ```
//! use stackable_certs::sign::Ed25519SigningKey;
//!
//! let key = Ed25519SigningKey::new();
//! let sig = key.sign(&[72, 69, 76, 76, 79]);
//! ```
//!
//! Additionally, this crate enables the `pkcs8` and `pem` features, which enable
//! PKCS8 and PEM serialization support.
//!
//! ## RSA
//!
//! This module currently does not provide an RSA implementation, because of the
//! recently discovered Marvin attack. Both the `rsa` and `openssl` crate are
//! impacted by this. Support for RSA based signing keys can be added, once the
//! attack has been mitigated. See:
//!
//! - <https://people.redhat.com/~hkario/marvin/>
//! - <https://rustsec.org/advisories/RUSTSEC-2023-0071.html>
//! - <https://github.com/RustCrypto/RSA/issues/19>

use std::ops::Deref;

use rand::rngs::OsRng;
use rand_core::CryptoRngCore;
use rsa::{pkcs1v15::SigningKey, RsaPrivateKey, RsaPublicKey};

/// A signing key based on the ED25519 algorithm.
pub struct Ed25519SigningKey(ed25519_dalek::SigningKey);

impl Deref for Ed25519SigningKey {
    type Target = ed25519_dalek::SigningKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Ed25519SigningKey {
    /// Creates a new ED25519 signing key with the default cryptographically
    /// secure pseudo-randum number generator [`OsRng`].
    ///
    /// ```
    /// use stackable_certs::sign::Ed25519SigningKey;
    /// let key = Ed25519SigningKey::new();
    /// ```
    pub fn new() -> Self {
        let mut csprng = OsRng;
        Self::new_with(&mut csprng)
    }

    /// Creates a new ED25519 signing key with a custom cryptographically
    /// secure pseudo-randum number generator. To use the default [`OsRng`]
    /// generator, use [`Ed25519SigningKey::new()`] instead.
    pub fn new_with<R>(csprng: &mut R) -> Self
    where
        R: CryptoRngCore + ?Sized,
    {
        let signing_key = ed25519_dalek::SigningKey::generate(csprng);
        Self(signing_key)
    }
}

#[derive(Debug)]
pub struct RsaSigningKey {
    pub private_key: RsaPrivateKey,
    pub public_key: RsaPublicKey,
}

impl RsaSigningKey {
    pub fn new() -> Self {
        let mut csprng = OsRng;
        Self::new_with(&mut csprng)
    }

    pub fn new_with<R>(csprng: &mut R) -> Self
    where
        R: CryptoRngCore + ?Sized,
    {
        println!("Here");
        // TODO (@Techassi): Remove unwrap
        let private_key = RsaPrivateKey::new(csprng, 2048).unwrap();
        let public_key = RsaPublicKey::from(&private_key);

        println!("Takes longs?");

        Self {
            private_key,
            public_key,
        }
    }
}

#[cfg(test)]
mod test {
    use ed25519_dalek::Signer;

    use super::*;

    #[test]
    fn test() {
        let key = Ed25519SigningKey::new();
        let sig = key.sign(&[72, 69, 76, 76, 79]);

        println!("{sig}")
    }
}
