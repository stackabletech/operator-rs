//! Contains structs and functions to easily create a TLS termination server,
//! which can be used in combination with an Axum [`Router`][axum::Router].
mod certs;
mod server;

pub use certs::*;
pub use server::*;
