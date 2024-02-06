use std::net::SocketAddr;

use crate::constants::{DEFAULT_HTTPS_PORT, DEFAULT_HTTP_PORT, DEFAULT_IP_ADDRESS};

/// Specifies available webhook server options.
///
/// The [`Default`] implemention for this struct contains the following
/// values:
///
/// - Redirect from HTTP to HTTPS is enabled, HTTP listens on port 8080
/// - The socket binds to 127.0.0.1 on port 8443 (HTTPS)
pub struct Options {
    /// Enables or disables the automatic HTTP to HTTPS redirect. If enabled,
    /// it is required to specify the HTTP port.
    pub redirect: RedirectOption,

    /// The default HTTPS socket address the [`TcpListener`] binds to. The same
    /// IP adress is used for the auto HTTP to HTTPS redirect handler.
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
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct OptionsBuilder {
    redirect: Option<RedirectOption>,
    socket_addr: Option<SocketAddr>,
    tls: Option<TlsOption>,
}

impl OptionsBuilder {
    pub fn redirect(mut self, redirect: RedirectOption) -> Self {
        self.redirect = Some(redirect);
        self
    }

    pub fn disable_redirect(self) -> Self {
        self.redirect(RedirectOption::Disabled)
    }

    pub fn enable_redirect(self, http_port: u16) -> Self {
        self.redirect(RedirectOption::Enabled(http_port))
    }

    pub fn socket_addr<T>(mut self, socket_addr: T) -> Self
    where
        T: Into<SocketAddr>,
    {
        self.socket_addr = Some(socket_addr.into());
        self
    }

    pub fn tls(mut self, tls: TlsOption) -> Self {
        self.tls = Some(tls);
        self
    }

    pub fn build(self) -> Options {
        Options {
            redirect: self.redirect.unwrap_or_default(),
            socket_addr: self
                .socket_addr
                .unwrap_or(SocketAddr::from((DEFAULT_IP_ADDRESS, DEFAULT_HTTPS_PORT))),
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
    Inject,
}

impl Default for TlsOption {
    fn default() -> Self {
        Self::AutoGenerate
    }
}
