//! ## Crate Features
//!
//! - `default` enables a default set of features which most operators need.
//! - `full` enables all available features.
//! - `time` enables interoperability between [`time::Duration`] and the `time` crate.
//! - `telemetry` enables various helpers for emitting telemetry data.
//! - `versioned` enables the macro for CRD versioning.

pub mod builder;
pub mod cli;
pub mod client;
pub mod cluster_resources;
pub mod commons;
pub mod config;
pub mod cpu;
pub mod crd;
pub mod helm;
pub mod iter;
pub mod kvp;
pub mod logging;
pub mod memory;
pub mod namespace;
pub mod pod_utils;
pub mod product_config_utils;
pub mod product_logging;
pub mod role_utils;
pub mod status;
pub mod time;
pub mod utils;
pub mod validation;

// External re-exports
pub use k8s_openapi;
pub use kube;
pub use schemars;
// Internal re-exports
// TODO (@Techassi): Ideally we would want webhook and certs exported here as
// well, but that would require some restructuring of crates.
pub use stackable_shared as shared;
pub use stackable_shared::{crd::CustomResourceExt, yaml::YamlSchema};
#[cfg(feature = "telemetry")]
pub use stackable_telemetry as telemetry;
#[cfg(feature = "versioned")]
pub use stackable_versioned as versioned;
