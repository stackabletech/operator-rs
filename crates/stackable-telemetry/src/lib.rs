#![warn(missing_docs)]

//! This crate contains various Tracing, Logging, and OpenTelemetry primitives to easily instrument
//! code.
pub mod instrumentation;
pub mod tracing;

pub use instrumentation::AxumTraceLayer;
pub use tracing::Tracing;
