use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::Host,
    handler::HandlerWithoutStateExt,
    http::{
        uri::{InvalidUri, InvalidUriParts, Scheme},
        StatusCode, Uri,
    },
    response::Redirect,
};
use snafu::{ResultExt, Snafu};
use tokio::net::TcpListener;
use tracing::warn;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse HTTPS host as authority"))]
    ParseAuthority { source: InvalidUri },

    #[snafu(display("failed to convert URI parts into URI"))]
    ConvertPartsToUri { source: InvalidUriParts },
}

/// A redirector which redirects HTTP connections at "/" to HTTPS automatically.
///
/// Internally it uses a simple handler function which is registered as a
/// singular [`Service`][tower::MakeService] at the root "/" path. If the
/// conversion from HTTP to HTTPS fails, the [`Redirector`] returns a HTTP
/// status code 400 (Bad Request). Additionally, a warning trace is emitted.
pub struct Redirector {
    ip_addr: IpAddr,
    https_port: u16,
    http_port: u16,
}

impl Redirector {
    pub fn new(ip_addr: IpAddr, https_port: u16, http_port: u16) -> Self {
        Self {
            https_port,
            http_port,
            ip_addr,
        }
    }

    pub async fn run(self) {
        // The redirector only binds to the HTTP port. The actual HTTPS
        // application runs in a separate task and is completely independent
        // of this redirector.
        let socket_addr = SocketAddr::new(self.ip_addr, self.http_port);
        let listener = TcpListener::bind(socket_addr).await.unwrap();

        // This converts the HTTP request URI into HTTPS. If this fails, the
        // redirector emits a warning trace and returns HTTP status code 400
        // (Bad Request).
        let redirect = move |Host(host): Host, uri: Uri| async move {
            match http_to_https(host, uri, self.http_port, self.https_port) {
                Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
                Err(err) => {
                    warn!(%err, "failed to convert HTTP URI to HTTPS");
                    Err(StatusCode::BAD_REQUEST)
                }
            }
        };

        // This registers the handler function as the only handler at the root
        // path "/". See https://docs.rs/axum/latest/axum/fn.serve.html#examples
        axum::serve(listener, redirect.into_make_service())
            .await
            .unwrap();
    }
}

fn http_to_https(host: String, uri: Uri, http_port: u16, https_port: u16) -> Result<Uri, Error> {
    let mut parts = uri.into_parts();

    parts.scheme = Some(Scheme::HTTPS);

    if parts.path_and_query.is_none() {
        // NOTE (@Techassi): This should never fail and is this save to unwrap.
        // If this will change into a user-controlled value, then this isn't
        // save to unwrap anymore and will require explicit error handling.
        parts.path_and_query = Some("/".parse().unwrap());
    }

    let https_host = host.replace(&http_port.to_string(), &https_port.to_string());
    parts.authority = Some(https_host.parse().context(ParseAuthoritySnafu)?);

    Ok(Uri::from_parts(parts).context(ConvertPartsToUriSnafu)?)
}
