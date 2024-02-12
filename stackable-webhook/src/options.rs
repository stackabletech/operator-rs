//! Contains available options to configure the [WebhookServer][crate::WebhookServer].
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use crate::{
    constants::{DEFAULT_HTTP_PORT, DEFAULT_SOCKET_ADDR},
    tls::PrivateKeyEncoding,
};

/// Specifies available webhook server options.
///
/// The [`Default`] implemention for this struct contains the following
/// values:
///
/// - Redirect from HTTP to HTTPS is enabled, HTTP listens on port 8080
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
///     .socket_addr([0, 0, 0, 0], 12345)
///     .build();
///
/// // Set IP address only
/// let options = Options::builder()
///     .socket_ip([0, 0, 0, 0])
///     .build();
///
/// // Set port only
/// let options = Options::builder()
///     .socket_port(12345)
///     .build();
/// ```
///
/// ### Example with Custom Redirects
///
/// ```
/// use stackable_webhook::Options;
///
/// // Use a custom HTTP port
/// let options = Options::builder()
///     .enable_redirect(12345)
///     .build();
///
/// // Disable auto-redirect
/// let options = Options::builder()
///     .disable_redirect()
///     .build();
/// ```
///
/// ### Example with Mounted TLS Certificate
///
/// ```
/// use stackable_webhook::Options;
///
/// let options = Options::builder()
///     .tls_mount("path/to/pem/cert", "path/to/pem/key")
///     .build();
/// ```
#[derive(Debug)]
pub struct Options {
    /// Enables or disables the automatic HTTP to HTTPS redirect. If enabled,
    /// it is required to specify the HTTP port. If disabled, the webhook
    /// server **only** listens on HTTPS.
    pub redirect: RedirectOption,

    /// The default HTTPS socket address the [`TcpListener`][tokio::net::TcpListener]
    /// binds to. The same IP adress is used for the auto HTTP to HTTPS redirect
    /// handler.
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
    redirect: Option<RedirectOption>,
    socket_addr: Option<SocketAddr>,
    tls: Option<TlsOption>,
}

impl OptionsBuilder {
    /// Disables HTPP to HTTPS auto-redirect entirely. The webhook server
    /// will only listen on HTTPS.
    pub fn disable_redirect(mut self) -> Self {
        self.redirect = Some(RedirectOption::Disabled);
        self
    }

    /// Enables HTTP to HTTPS auto-redirect on `http_port`. The webhook
    /// server will listen on both HTTP and HTTPS.
    pub fn enable_redirect(mut self, http_port: u16) -> Self {
        self.redirect = Some(RedirectOption::Enabled(http_port));
        self
    }

    /// Sets the socket address the webhook server uses to bind for HTTPS.
    pub fn socket_addr(mut self, socket_ip: impl Into<IpAddr>, socket_port: u16) -> Self {
        self.socket_addr = Some(SocketAddr::new(socket_ip.into(), socket_port));
        self
    }

    /// Sets the IP address of the socket address the webhook server uses to
    /// bind for HTTPS.
    pub fn socket_ip(mut self, socket_ip: impl Into<IpAddr>) -> Self {
        let addr = self.socket_addr.get_or_insert(DEFAULT_SOCKET_ADDR);
        addr.set_ip(socket_ip.into());
        self
    }

    /// Sets the port of the socket address the webhook server uses to bind
    /// for HTTPS.
    pub fn socket_port(mut self, socket_port: u16) -> Self {
        let addr = self.socket_addr.get_or_insert(DEFAULT_SOCKET_ADDR);
        addr.set_port(socket_port);
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
        cert_path: impl Into<PathBuf>,
        pk_path: impl Into<PathBuf>,
        pk_encoding: PrivateKeyEncoding,
    ) -> Self {
        self.tls = Some(TlsOption::Mount {
            cert_path: cert_path.into(),
            pk_path: pk_path.into(),
            pk_encoding,
        });
        self
    }

    /// Builds the final [`Options`] by using default values for any not
    /// explicitly set option.
    pub fn build(self) -> Options {
        Options {
            redirect: self.redirect.unwrap_or_default(),
            socket_addr: self.socket_addr.unwrap_or(DEFAULT_SOCKET_ADDR),
            tls: self.tls.unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub enum RedirectOption {
    Enabled(u16),
    Disabled,
}

impl Default for RedirectOption {
    fn default() -> Self {
        Self::Enabled(DEFAULT_HTTP_PORT)
    }
}

#[derive(Debug)]
pub enum TlsOption {
    AutoGenerate,
    Mount {
        pk_encoding: PrivateKeyEncoding,
        cert_path: PathBuf,
        pk_path: PathBuf,
    },
}

impl Default for TlsOption {
    fn default() -> Self {
        Self::AutoGenerate
    }
}
