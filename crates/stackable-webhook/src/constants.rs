//! Contains various constant definitions, mostly for default ports and IP
//! addresses.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// The default HTTPS port `8443`
pub const DEFAULT_HTTPS_PORT: u16 = 8443;

/// The default IP address `127.0.0.1` the webhook server binds to.
pub const DEFAULT_IP_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

/// The default socket address `127.0.0.1:8443` the webhook server binds to.
pub const DEFAULT_SOCKET_ADDR: SocketAddr = SocketAddr::new(DEFAULT_IP_ADDRESS, DEFAULT_HTTPS_PORT);
