//! Contains various constant definitions, mostly for default ports and IP
//! addresses.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// The default HTTPS port `8443`
pub const DEFAULT_HTTPS_PORT: u16 = 8443;

/// The default IP address [`Ipv4Addr::UNSPECIFIED`] (`0.0.0.0`) the webhook server binds to,
/// which represents binding on all network addresses.
//
// TODO: We might want to switch to `Ipv6Addr::UNSPECIFIED)` here, as this *normally* binds to IPv4
// and IPv6. However, it's complicated and depends on the underlying system...
// If we do so, we should set `set_only_v6(false)` on the socket to not rely on system defaults.
pub const DEFAULT_LISTEN_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

/// The default socket address `0.0.0.0:8443` the webhook server binds to.
pub const DEFAULT_SOCKET_ADDRESS: SocketAddr =
    SocketAddr::new(DEFAULT_LISTEN_ADDRESS, DEFAULT_HTTPS_PORT);
