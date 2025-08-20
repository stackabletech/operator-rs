//! Contains available options to configure the [WebhookServer][crate::WebhookServer].
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use stackable_certs::PrivateKeyType;

use crate::constants::DEFAULT_SOCKET_ADDRESS;

/// Specifies available webhook server options.
///
/// The [`Default`] implementation for this struct contains the following values:
///
/// - The socket binds to 127.0.0.1 on port 8443 (HTTPS)
/// - An empty list of SANs is provided to the certificate the TLS server uses.
///
/// ### Example with Custom HTTPS IP Address and Port
///
/// ```
/// use stackable_webhook::WebhookOptions;
///
/// // Set IP address and port at the same time
/// let options = WebhookOptions::builder()
///     .bind_address([0, 0, 0, 0], 12345)
///     .build();
///
/// // Set IP address only
/// let options = WebhookOptions::builder()
///     .bind_ip([0, 0, 0, 0])
///     .build();
///
/// // Set port only
/// let options = WebhookOptions::builder()
///     .bind_port(12345)
///     .build();
/// ```
#[derive(Debug)]
pub struct WebhookOptions {
    /// The default HTTPS socket address the [`TcpListener`][tokio::net::TcpListener]
    /// binds to.
    pub socket_addr: SocketAddr,

    /// The subject alterative DNS names that should be added to the certificates generated for this
    /// webhook.
    pub subject_alterative_dns_names: Vec<String>,
}

impl Default for WebhookOptions {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl WebhookOptions {
    /// Returns the default [`WebhookOptionsBuilder`] which allows to selectively
    /// customize the options. See the documentation for [`WebhookOptions`] for more
    /// information on available functions.
    pub fn builder() -> WebhookOptionsBuilder {
        WebhookOptionsBuilder::default()
    }
}

/// The [`WebhookOptionsBuilder`] which allows to selectively customize the webhook
/// server [`WebhookOptions`].
///
/// Usually, this struct is not constructed manually, but instead by calling
/// [`WebhookOptions::builder()`] or [`WebhookOptionsBuilder::default()`].
#[derive(Debug, Default)]
pub struct WebhookOptionsBuilder {
    socket_addr: Option<SocketAddr>,
    subject_alterative_dns_names: Vec<String>,
}

impl WebhookOptionsBuilder {
    /// Sets the socket address the webhook server uses to bind for HTTPS.
    pub fn bind_address(mut self, bind_ip: impl Into<IpAddr>, bind_port: u16) -> Self {
        self.socket_addr = Some(SocketAddr::new(bind_ip.into(), bind_port));
        self
    }

    /// Sets the IP address of the socket address the webhook server uses to
    /// bind for HTTPS.
    pub fn bind_ip(mut self, bind_ip: impl Into<IpAddr>) -> Self {
        let addr = self.socket_addr.get_or_insert(DEFAULT_SOCKET_ADDRESS);
        addr.set_ip(bind_ip.into());
        self
    }

    /// Sets the port of the socket address the webhook server uses to bind
    /// for HTTPS.
    pub fn bind_port(mut self, bind_port: u16) -> Self {
        let addr = self.socket_addr.get_or_insert(DEFAULT_SOCKET_ADDRESS);
        addr.set_port(bind_port);
        self
    }

    /// Sets the subject alterative DNS names that should be added to the certificates generated for
    /// this webhook.
    pub fn subject_alterative_dns_names(
        mut self,
        subject_alterative_dns_name: Vec<String>,
    ) -> Self {
        self.subject_alterative_dns_names = subject_alterative_dns_name;
        self
    }

    /// Adds the subject alterative DNS name to the list of names.
    pub fn add_subject_alterative_dns_name(
        mut self,
        subject_alterative_dns_name: impl Into<String>,
    ) -> Self {
        self.subject_alterative_dns_names
            .push(subject_alterative_dns_name.into());
        self
    }

    /// Builds the final [`WebhookOptions`] by using default values for any not
    /// explicitly set option.
    pub fn build(self) -> WebhookOptions {
        WebhookOptions {
            socket_addr: self.socket_addr.unwrap_or(DEFAULT_SOCKET_ADDRESS),
            subject_alterative_dns_names: self.subject_alterative_dns_names,
        }
    }
}

#[derive(Debug)]
pub enum TlsOption {
    AutoGenerate,
    Mount {
        private_key_type: PrivateKeyType,
        private_key_path: PathBuf,
        certificate_path: PathBuf,
    },
}

impl Default for TlsOption {
    fn default() -> Self {
        Self::AutoGenerate
    }
}
