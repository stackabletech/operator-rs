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
pub mod config_overrides;
pub mod constants;
pub mod cpu;
pub mod crd;
pub mod database_connections;
pub mod deep_merger;
pub mod eos;
pub mod helm;
pub mod iter;
pub mod kvp;
pub mod logging;
pub mod memory;
pub mod namespace;
pub mod pod_utils;
pub mod product_logging;
pub mod status;
pub mod test_utils;
pub mod utils;
pub mod v2;
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

/// This macro can be used to gate every contained item behind `feature`.
///
/// ```ignore
/// use crate::cfg_block;
///
/// cfg_block! {
///     "crds";
///
///     pub use foo;
///     pub use bar;
/// }
/// ```
#[macro_export]
macro_rules! cfg_block {
    ($feature:literal; $($item:item)*) => {
        $(
            #[cfg(feature = $feature)]
            $item
        )*
    }
}
