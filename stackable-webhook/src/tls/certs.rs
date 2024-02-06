use std::{fs::File, io::BufReader, path::Path};

use rustls_pemfile::{certs, pkcs8_private_keys};
use snafu::{ResultExt, Snafu};
use tokio_rustls::rustls::{Certificate, PrivateKey};

#[derive(Debug, Snafu)]
pub enum Error {
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

impl<C, P> TryFrom<(&mut C, &mut P)> for CertificateChain
where
    C: std::io::BufRead,
    P: std::io::BufRead,
{
    type Error = Error;

    fn try_from(readers: (&mut C, &mut P)) -> Result<Self, Self::Error> {
        let chain = certs(readers.0)
            .context(ReadBufferedCertFileSnafu)?
            .into_iter()
            .map(Certificate)
            .collect();

        let private_key = pkcs8_private_keys(readers.1)
            .context(ReadBufferedKeyFileSnafu)?
            .remove(0);
        let private_key = PrivateKey(private_key);

        Ok(Self { chain, private_key })
    }
}

impl CertificateChain {
    pub fn from_files<C, P>(certificate_path: C, private_key_path: P) -> Result<Self, Error>
    where
        C: AsRef<Path>,
        P: AsRef<Path>,
    {
        let cert_file = File::open(certificate_path).context(ReadCertFileSnafu)?;
        let cert_reader = &mut BufReader::new(cert_file);

        let key_file = File::open(private_key_path).context(ReadKeyFileSnafu)?;
        let key_reader = &mut BufReader::new(key_file);

        Self::try_from((cert_reader, key_reader))
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
