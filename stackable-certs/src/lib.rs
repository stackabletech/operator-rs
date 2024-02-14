use std::{fs::File, io::BufReader, path::Path};

use rustls_pemfile::{certs, ec_private_keys, pkcs8_private_keys, rsa_private_keys};
use snafu::{ResultExt, Snafu};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

pub mod ca;

pub type Result<T, E = CertifacteError> = std::result::Result<T, E>;

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
    chain: Vec<CertificateDer<'static>>,
    private_key: PrivateKeyDer<'static>,
}

impl CertificateChain {
    pub fn from_files(
        cert_path: impl AsRef<Path>,
        pk_path: impl AsRef<Path>,
        pk_encoding: PrivateKeyEncoding,
    ) -> Result<Self> {
        let cert_file = File::open(cert_path).context(ReadCertFileSnafu)?;
        let mut cert_reader = BufReader::new(cert_file);

        let key_file = File::open(pk_path).context(ReadKeyFileSnafu)?;
        let mut pk_reader = BufReader::new(key_file);

        let chain = certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .context(ReadBufferedCertFileSnafu)?;

        let private_key = match pk_encoding {
            PrivateKeyEncoding::Pkcs8 => Self::pkcs8_to_pk_der(&mut pk_reader)?,
            PrivateKeyEncoding::Rsa => Self::rsa_to_pk_der(&mut pk_reader)?,
            PrivateKeyEncoding::Ec => Self::ec_to_pk_der(&mut pk_reader)?,
        }
        .remove(0);

        Ok(Self { chain, private_key })
    }

    fn pkcs8_to_pk_der<'a>(pk_reader: &mut dyn std::io::BufRead) -> Result<Vec<PrivateKeyDer<'a>>> {
        let ders = pkcs8_private_keys(pk_reader)
            .collect::<Result<Vec<_>, _>>()
            .context(ReadBufferedKeyFileSnafu)?;

        Ok(ders.into_iter().map(PrivateKeyDer::from).collect())
    }

    fn rsa_to_pk_der<'a>(pk_reader: &mut dyn std::io::BufRead) -> Result<Vec<PrivateKeyDer<'a>>> {
        let ders = rsa_private_keys(pk_reader)
            .collect::<Result<Vec<_>, _>>()
            .context(ReadBufferedKeyFileSnafu)?;

        Ok(ders.into_iter().map(PrivateKeyDer::from).collect())
    }

    fn ec_to_pk_der<'a>(pk_reader: &mut dyn std::io::BufRead) -> Result<Vec<PrivateKeyDer<'a>>> {
        let ders = ec_private_keys(pk_reader)
            .collect::<Result<Vec<_>, _>>()
            .context(ReadBufferedKeyFileSnafu)?;

        Ok(ders.into_iter().map(PrivateKeyDer::from).collect())
    }

    pub fn chain(&self) -> &[CertificateDer] {
        &self.chain
    }

    pub fn private_key(&self) -> &PrivateKeyDer {
        &self.private_key
    }

    pub fn into_parts(self) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
        (self.chain, self.private_key)
    }
}

#[derive(Debug)]
pub enum PrivateKeyEncoding {
    Pkcs8,
    Rsa,
    Ec,
}
