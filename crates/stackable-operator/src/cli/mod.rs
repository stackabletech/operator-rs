//! This module provides helper methods to deal with common CLI options using the `clap` crate.
//!
//! In particular it currently supports handling two kinds of options:
//! * CRD printing
//! * Product config location
//!
//! # Example
//!
//! This example show the usage of the CRD functionality.
//!
//! ```no_run
//! // Handle CLI arguments
//! use clap::{crate_version, Parser};
//! use kube::CustomResource;
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//! use stackable_operator::{CustomResourceExt, cli, shared::crd};
//!
//! const OPERATOR_VERSION: &str = "23.1.1";
//!
//! #[derive(Clone, CustomResource, Debug, JsonSchema, Serialize, Deserialize)]
//! #[kube(
//!     group = "foo.stackable.tech",
//!     version = "v1",
//!     kind = "FooCluster",
//!     namespaced
//! )]
//! pub struct FooClusterSpec {
//!     pub name: String,
//! }
//!
//! #[derive(Clone, CustomResource, Debug, JsonSchema, Serialize, Deserialize)]
//! #[kube(
//!     group = "bar.stackable.tech",
//!     version = "v1",
//!     kind = "BarCluster",
//!     namespaced
//! )]
//! pub struct BarClusterSpec {
//!     pub name: String,
//! }
//!
//! #[derive(clap::Parser)]
//! #[command(
//!     name = "Foobar Operator",
//!     author,
//!     version,
//!     about = "Stackable Operator for Foobar"
//! )]
//! struct Opts {
//!     #[clap(subcommand)]
//!     command: cli::Command,
//! }
//!
//! # fn main() -> Result<(), crd::Error> {
//! let opts = Opts::parse();
//!
//! match opts.command {
//!     cli::Command::Crd => {
//!         FooCluster::print_yaml_schema(OPERATOR_VERSION)?;
//!         BarCluster::print_yaml_schema(OPERATOR_VERSION)?;
//!     },
//!     cli::Command::Run { .. } => {
//!         // Run the operator
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Product config handling works similarly:
//!
//! ```no_run
//! use clap::{crate_version, Parser};
//! use stackable_operator::cli;
//!
//! #[derive(clap::Parser)]
//! #[command(
//!     name = "Foobar Operator",
//!     author,
//!     version,
//!     about = "Stackable Operator for Foobar"
//! )]
//! struct Opts {
//!     #[clap(subcommand)]
//!     command: cli::Command,
//! }
//!
//! # fn main() -> Result<(), cli::Error> {
//! let opts = Opts::parse();
//!
//! match opts.command {
//!     cli::Command::Crd => {
//!         // Print CRD objects
//!     }
//!     cli::Command::Run(cli::ProductOperatorRun { product_config, watch_namespace, .. }) => {
//!         let product_config = product_config.load(&[
//!             "deploy/config-spec/properties.yaml",
//!             "/etc/stackable/spark-operator/config-spec/properties.yaml",
//!         ])?;
//!     }
//! }
//! # Ok(())
//! # }
//!
//! ```
//!
//!
use clap::{Args, Parser};
use stackable_telemetry::tracing::TelemetryOptions;

use crate::{namespace::WatchNamespace, utils::cluster_info::KubernetesClusterInfoOptions};

mod environment;
mod maintenance;
mod product_config;

pub use environment::*;
pub use maintenance::*;
pub use product_config::*;

// NOTE (@Techassi): Why the hell is this here? Let's get rid of it.
pub const AUTHOR: &str = "Stackable GmbH - info@stackable.tech";

/// A common set of commands used by operators.
///
/// This enum is generic over the arguments available to the [`Command::Run`] subcommand. By default,
/// [`RunArguments`] is used, but a custom type can be used.
///
/// ```rust
/// use stackable_operator::cli::Command;
/// use clap::Parser;
///
/// #[derive(Parser)]
/// struct Run {
///     #[arg(long)]
///     name: String,
/// }
///
/// let _ = Command::<Run>::parse_from(["foobar-operator", "run", "--name", "foo"]);
/// ```
///
/// If you need operator-specific commands then you can flatten [`Command`] into your own command
/// enum.
///
/// ```rust
/// use stackable_operator::cli::Command;
/// use clap::Parser;
///
/// #[derive(Parser)]
/// enum Command {
///     /// Print hello world message
///     Hello,
///
///     #[clap(flatten)]
///     Framework(Command)
/// }
/// ```
#[derive(Debug, PartialEq, Eq, Parser)]
pub enum Command<Run: Args = RunArguments> {
    /// Print CRD objects.
    Crd,

    /// Run the operator.
    Run(Run),
}

/// Default CLI arguments that most operators take when running.
///
/// ### Embed into an extended argument set
///
/// ```rust
/// use stackable_operator::cli::RunArguments;
/// use clap::Parser;
///
/// #[derive(clap::Parser, Debug, PartialEq, Eq)]
/// struct Run {
///     #[clap(long)]
///     name: String,
///
///     #[clap(flatten)]
///     common: RunArguments,
/// }
/// ```
#[derive(Debug, PartialEq, Eq, Parser)]
#[command(long_about = "")]
pub struct RunArguments {
    /// Provides the path to a product-config file
    #[arg(long, short = 'p', value_name = "FILE", default_value = "", env)]
    pub product_config: ProductConfigPath,

    // TODO (@Techassi): This should be moved into the environment options
    /// Provides a specific namespace to watch (instead of watching all namespaces)
    #[arg(long, env, default_value = "")]
    pub watch_namespace: WatchNamespace,

    // IMPORTANT: All (flattened) sub structs should be placed at the end to ensure the help
    // headings are correct.
    #[command(flatten)]
    pub common: CommonOptions,

    #[command(flatten)]
    pub maintenance: MaintenanceOptions,

    #[command(flatten)]
    pub operator_environment: OperatorEnvironmentOptions,
}

/// A set of CLI arguments that all (or at least most) Stackable applications use.
///
/// [`RunArguments`] is intended for operators, but it has fields that are not needed for utilities
/// such as `user-info-fetcher` or `opa-bundle-builder`. So this struct offers a limited set, that
/// should be shared across all Stackable tools running on Kubernetes.
#[derive(Debug, PartialEq, Eq, Args)]
pub struct CommonOptions {
    #[command(flatten)]
    pub telemetry: TelemetryOptions,

    #[command(flatten)]
    pub cluster_info: KubernetesClusterInfoOptions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;

        RunArguments::command().print_long_help().unwrap();
        RunArguments::command().debug_assert()
    }
}
