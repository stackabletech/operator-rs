//! Contains available options to configure the [WebhookServer][crate::WebhookServer].
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use stackable_certs::PrivateKeyEncoding;

use crate::constants::DEFAULT_SOCKET_ADDR;

/// Specifies available webhook server options.
///
/// The [`Default`] implemention for this struct contains the following
/// values:
///
/// - The socket binds to 127.0.0.1 on port 8443 (HTTPS)
/// - The TLS cert used gets auto-generated
///
/// ### Example with Custom HTTPS IP Address and Port
///
/// ```
/// use stackable_webhook::Options;
///
/// // Set IP address and port at the same time
/// let options = Options::builder()
///     .bind_address([0, 0, 0, 0], 12345)
///     .build();
///
/// // Set IP address only
/// let options = Options::builder()
///     .bind_ip([0, 0, 0, 0])
///     .build();
///
/// // Set port only
/// let options = Options::builder()
///     .bind_port(12345)
///     .build();
/// ```
///
/// ### Example with Mounted TLS Certificate
///
/// ```
/// use stackable_certs::PrivateKeyEncoding;
/// use stackable_webhook::{Options};
///
/// let options = Options::builder()
///     .tls_mount(
///         "path/to/pem/cert",
///         "path/to/pem/key",
///         PrivateKeyEncoding::Pkcs8,
///     )
///     .build();
/// ```
#[derive(Debug)]
pub struct Options {
    /// The default HTTPS socket address the [`TcpListener`][tokio::net::TcpListener]
    /// binds to.
    pub socket_addr: SocketAddr,

    /// Either auto-generate or use an injected TLS certificate.
    pub tls: TlsOption,
}

impl Default for Options {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Options {
    /// Returns the default [`OptionsBuilder`] which allows to selectively
    /// customize the options. See the documention for [`Options`] for more
    /// information on available functions.
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::default()
    }
}

/// The [`OptionsBuilder`] which allows to selectively customize the webhook
/// server [`Options`].
///
/// Usually, this struct is not constructed manually, but instead by calling
/// [`Options::builder()`] or [`OptionsBuilder::default()`].
#[derive(Debug, Default)]
pub struct OptionsBuilder {
    socket_addr: Option<SocketAddr>,
    tls: Option<TlsOption>,
}

impl OptionsBuilder {
    /// Sets the socket address the webhook server uses to bind for HTTPS.
    pub fn bind_address(mut self, bind_ip: impl Into<IpAddr>, bind_port: u16) -> Self {
        self.socket_addr = Some(SocketAddr::new(bind_ip.into(), bind_port));
        self
    }

    /// Sets the IP address of the socket address the webhook server uses to
    /// bind for HTTPS.
    pub fn bind_ip(mut self, bind_ip: impl Into<IpAddr>) -> Self {
        let addr = self.socket_addr.get_or_insert(DEFAULT_SOCKET_ADDR);
        addr.set_ip(bind_ip.into());
        self
    }

    /// Sets the port of the socket address the webhook server uses to bind
    /// for HTTPS.
    pub fn bind_port(mut self, bind_port: u16) -> Self {
        let addr = self.socket_addr.get_or_insert(DEFAULT_SOCKET_ADDR);
        addr.set_port(bind_port);
        self
    }

    /// Enables TLS certificate auto-generation instead of using a mounted
    /// one. If instead a mounted TLS certificate is needed, use the
    /// [`OptionsBuilder::tls_mount()`] function.
    pub fn tls_autogenerate(mut self) -> Self {
        self.tls = Some(TlsOption::AutoGenerate);
        self
    }

    /// Uses a mounted TLS certificate instead of auto-generating one. If
    /// instead a auto-generated TLS certificate is needed, us ethe
    /// [`OptionsBuilder::tls_autogenerate()`] function.
    pub fn tls_mount(
        mut self,
        public_key_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
        private_key_encoding: PrivateKeyEncoding,
    ) -> Self {
        self.tls = Some(TlsOption::Mount {
            private_key_path: public_key_path.into(),
            certificate_path: private_key_path.into(),
            private_key_encoding,
        });
        self
    }

    /// Builds the final [`Options`] by using default values for any not
    /// explicitly set option.
    pub fn build(self) -> Options {
        Options {
            socket_addr: self.socket_addr.unwrap_or(DEFAULT_SOCKET_ADDR),
            tls: self.tls.unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub enum TlsOption {
    AutoGenerate,
    Mount {
        private_key_encoding: PrivateKeyEncoding,
        private_key_path: PathBuf,
        certificate_path: PathBuf,
    },
}

impl Default for TlsOption {
    fn default() -> Self {
        Self::AutoGenerate
    }
}
