use std::path::Path;

use x509_cert::der::pem::LineEnding;

pub mod ca;
pub mod chain;
pub mod sign;

pub use chain::*;

// TODO (@Techassi): Maybe add functions to read/write certificates from/to K8s
// secrets or separate those out into a K8s specific trait.
pub trait CertificateExt: Sized {
    const CERTIFICATE_FILE_EXT: &'static str = "pem";
    const PRIVATE_KEY_FILE_EXT: &'static str = "pk8";

    type Error: std::error::Error;

    /// Reads in a PEM-encoded certificate from `certificate_path` and private
    /// key file from `private_key_path` and finally constructs a CA from the
    /// contents.
    fn from_files(
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
    ) -> Result<Self, Self::Error>;

    /// Writes the certificate and private key as a PEM-encoded file to
    /// `certificate_path` and `private_key_path` respectively.
    ///
    /// This function will always use [`Self::CERTIFICATE_FILE_EXT`] for the
    /// certificate and [`Self::PRIVATE_KEY_FILE_EXT`] for the private key
    /// file extension.
    fn to_files(
        &self,
        certificate_path: impl AsRef<Path>,
        private_key_path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<(), Self::Error>;
}

pub trait K8sCertificateExt: CertificateExt {
    // TODO (@Techassi): Use SecretReference here, for that, we would need to
    // move it out of secret-operator into a common place.
    fn from_secret(client: (), secret_ref: &str);
}

pub trait SecretExt {
    type Error: std::error::Error;

    fn requires_renewal(&self) -> bool;
    fn renew(&mut self, renew_after: u64) -> Result<(), Self::Error>;
}
