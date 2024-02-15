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

use std::ops::Deref;

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand_core::CryptoRngCore;

/// A signing key based on the ED25519 algorithm.
pub struct Ed25519SigningKey(SigningKey);

impl Deref for Ed25519SigningKey {
    type Target = SigningKey;

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
        let signing_key = SigningKey::generate(csprng);
        Self(signing_key)
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
