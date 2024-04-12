//! Auto-instrumentation of various utilities like HTTP servers and clients.
pub mod axum;

pub use axum::TraceLayer as AxumTraceLayer;
