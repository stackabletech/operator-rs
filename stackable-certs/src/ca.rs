// NOTE (@Techassi): Not all this code should live here, there will be other
// modules which handle a subset of features. For now this mostly serves as
// a rough scratch pad to sketch out the general pieces of code and get more
// comfortable with the x509_cert crate.
use snafu::Snafu;
use x509_cert::{time::Validity, Certificate};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {}

// TODO (@Techassi): Use correct types, these are mostly just placeholders
// TODO (@Techassi): Add a CA builder

/// A certificate authority (CA) which is used to generate and sign
/// intermediate certificates.
#[derive(Debug)]
pub struct CertificateAuthority {
    certificate: String,
    private_key: String,
    validity: Validity,
    subject: String,
    issuer: String,
}

impl CertificateAuthority {
    pub fn new() -> Self {
        todo!()
    }

    pub fn generate_intermediate_certificate(&self) -> Result<IntermediateCertificate> {
        todo!()
    }
}

// TODO (@Techassi): Use correct types, these are mostly just placeholders
#[derive(Debug)]
pub struct IntermediateCertificate {
    certificate: String,
    private_key: String,
    validity: Validity,
}

impl IntermediateCertificate {
    pub fn generate_leaf_certificate(&self) -> Result<Certificate> {
        todo!()
    }
}

// TODO (@Techassi): Maybe add functions to read/write certificates from/to K8s
// secrets or separate those out into a K8s specific trait.
pub trait CertificateExt: Sized {
    fn from_file() -> Result<Self>;
    fn into_file(self) -> Result<()>;
}
