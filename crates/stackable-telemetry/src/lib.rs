//! This crate contains various Tracing, Logging, and OpenTelemtry primitives to
//! easily instrument code.
pub mod instrumentation;
pub mod telemetry;

pub use instrumentation::AxumTraceLayer;
pub use telemetry::Telemetry;
