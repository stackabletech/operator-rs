//! Contains various types for composing the CLI interface for operators and other applications
//! running in a Kubernetes cluster.

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
/// enum CustomCommand {
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
