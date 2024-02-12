// TODO (@Techassi): Move this into a separate crate which handles TLS cert
// generation and reading.
use std::{fs::File, io::BufReader, path::Path};

use rustls_pemfile::{certs, ec_private_keys, pkcs8_private_keys, rsa_private_keys};
use snafu::{ResultExt, Snafu};
use tokio_rustls::rustls::{Certificate, PrivateKey};

#[derive(Debug, Snafu)]
pub enum CertifacteError {
    #[snafu(display("failed to read certificate file"))]
    ReadCertFile { source: std::io::Error },

    #[snafu(display("failed to read buffered certificate file"))]
    ReadBufferedCertFile { source: std::io::Error },

    #[snafu(display("failed to read private key file"))]
    ReadKeyFile { source: std::io::Error },

    #[snafu(display("failed to read buffered private key file"))]
    ReadBufferedKeyFile { source: std::io::Error },
}

pub struct CertificateChain {
    chain: Vec<Certificate>,
    private_key: PrivateKey,
}

impl CertificateChain {
    pub fn from_files(
        cert_path: impl AsRef<Path>,
        pk_path: impl AsRef<Path>,
        pk_encoding: PrivateKeyEncoding,
    ) -> Result<Self, CertifacteError> {
        let cert_file = File::open(cert_path).context(ReadCertFileSnafu)?;
        let cert_reader = &mut BufReader::new(cert_file);

        let key_file = File::open(pk_path).context(ReadKeyFileSnafu)?;
        let key_reader = &mut BufReader::new(key_file);

        Self::from_buffer(cert_reader, key_reader, pk_encoding)
    }

    fn from_buffer(
        cert_reader: &mut dyn std::io::BufRead,
        pk_reader: &mut dyn std::io::BufRead,
        pk_encoding: PrivateKeyEncoding,
    ) -> Result<Self, CertifacteError> {
        let chain = certs(cert_reader)
            .context(ReadBufferedCertFileSnafu)?
            .into_iter()
            .map(Certificate)
            .collect();

        let pk_bytes = match pk_encoding {
            PrivateKeyEncoding::Pkcs8 => pkcs8_private_keys(pk_reader),
            PrivateKeyEncoding::Rsa => rsa_private_keys(pk_reader),
            PrivateKeyEncoding::Ec => ec_private_keys(pk_reader),
        }
        .context(ReadBufferedKeyFileSnafu)?
        .remove(0);

        let private_key = PrivateKey(pk_bytes);
        Ok(Self { chain, private_key })
    }

    pub fn chain(&self) -> &[Certificate] {
        &self.chain
    }

    pub fn private_key(&self) -> &PrivateKey {
        &self.private_key
    }

    pub fn into_parts(self) -> (Vec<Certificate>, PrivateKey) {
        (self.chain, self.private_key)
    }
}

#[derive(Debug)]
pub enum PrivateKeyEncoding {
    Pkcs8,
    Rsa,
    Ec,
}
