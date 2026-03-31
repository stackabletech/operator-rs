#![deny(clippy::pedantic)]
#![expect(clippy::doc_markdown)]
#![expect(clippy::missing_errors_doc)]
#![expect(clippy::must_use_candidate)]
#![expect(clippy::return_self_not_must_use)]
#![expect(clippy::too_many_lines)]
#![expect(clippy::implicit_hasher)]
#![expect(clippy::doc_link_with_quotes)]
#![expect(clippy::missing_panics_doc)]
#![expect(clippy::explicit_deref_methods)]
#![expect(clippy::cast_possible_truncation)]
#![expect(clippy::float_cmp)]
#![expect(clippy::cast_sign_loss)]
#![expect(clippy::cast_precision_loss)]

//! ## Crate Features
//!
//! - `default` enables a default set of features which most operators need.
//! - `full` enables all available features.
//! - `time` enables interoperability between [`shared::time::Duration`] and the `time` crate.
//! - `telemetry` enables various helpers for emitting telemetry data.
//! - `versioned` enables the macro for CRD versioning.

pub mod builder;
pub mod cli;
pub mod client;
pub mod cluster_resources;
pub mod commons;
pub mod config;
pub mod constants;
pub mod cpu;
#[cfg(feature = "crds")]
pub mod crd;
pub mod deep_merger;
pub mod eos;
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
pub mod utils;
pub mod validation;

// External re-exports
pub use k8s_openapi;
pub use kube;
pub use schemars;
// Internal re-exports
// TODO (@Techassi): Ideally we would want webhook and certs exported here as
// well, but that would require some restructuring of crates.
#[cfg(feature = "certs")]
pub use stackable_certs as certs;
pub use stackable_shared as shared;
pub use stackable_shared::{crd::CustomResourceExt, yaml::YamlSchema};
pub use stackable_telemetry as telemetry;
#[cfg(feature = "crds")]
pub use stackable_versioned as versioned;
#[cfg(feature = "webhook")]
pub use stackable_webhook as webhook;
