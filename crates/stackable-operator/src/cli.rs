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
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use clap::Args;
use product_config::ProductConfigManager;
use snafu::{ResultExt, Snafu};
use stackable_telemetry::tracing::TelemetryOptions;

use crate::{namespace::WatchNamespace, utils::cluster_info::KubernetesClusterInfoOptions};

pub const AUTHOR: &str = "Stackable GmbH - info@stackable.tech";

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("failed to load product config"))]
    ProductConfigLoad {
        source: product_config::error::Error,
    },

    #[snafu(display(
        "failed to locate a required file in any of the following locations: {search_path:?}"
    ))]
    RequiredFileMissing { search_path: Vec<PathBuf> },
}

/// Framework-standardized commands
///
/// If you need operator-specific commands then you can flatten [`Command`] into your own command enum. For example:
/// ```rust
/// #[derive(clap::Parser)]
/// enum Command {
///     /// Print hello world message
///     Hello,
///     #[clap(flatten)]
///     Framework(stackable_operator::cli::Command)
/// }
/// ```
#[derive(clap::Parser, Debug, PartialEq, Eq)]
// The enum-level doccomment is intended for developers, not end users
// so supress it from being included in --help
#[command(long_about = "")]
pub enum Command<Run: Args = ProductOperatorRun> {
    /// Print CRD objects
    Crd,
    /// Run operator
    Run(Run),
}

/// Default parameters that all product operators take when running
///
/// Can be embedded into an extended argument set:
///
/// ```rust
/// # use stackable_operator::cli::{Command, CommonOptions, OperatorEnvironmentOptions, ProductOperatorRun, ProductConfigPath};
/// # use stackable_operator::{namespace::WatchNamespace, utils::cluster_info::KubernetesClusterInfoOptions};
/// # use stackable_telemetry::tracing::TelemetryOptions;
/// use clap::Parser;
///
/// #[derive(clap::Parser, Debug, PartialEq, Eq)]
/// struct Run {
///     #[clap(long)]
///     name: String,
///     #[clap(flatten)]
///     common: ProductOperatorRun,
/// }
///
/// let opts = Command::<Run>::parse_from([
///     "foobar-operator",
///     "run",
///     "--name",
///     "foo",
///     "--product-config",
///     "bar",
///     "--watch-namespace",
///     "foobar",
///     "--operator-namespace",
///     "stackable-operators",
///     "--operator-service-name",
///     "foo-operator",
///     "--kubernetes-node-name",
///     "baz",
/// ]);
/// assert_eq!(opts, Command::Run(Run {
///     name: "foo".to_string(),
///     common: ProductOperatorRun {
///         common: CommonOptions {
///             telemetry: TelemetryOptions::default(),
///             cluster_info: KubernetesClusterInfoOptions {
///                 kubernetes_cluster_domain: None,
///                 kubernetes_node_name: "baz".to_string(),
///             },
///         },
///         product_config: ProductConfigPath::from("bar".as_ref()),
///         watch_namespace: WatchNamespace::One("foobar".to_string()),
///         operator_environment: OperatorEnvironmentOptions {
///             operator_namespace: "stackable-operators".to_string(),
///             operator_service_name: "foo-operator".to_string(),
///         },
///     },
/// }));
/// ```
///
/// or replaced entirely
///
/// ```rust
/// # use stackable_operator::cli::{Command, ProductOperatorRun};
/// use clap::Parser;
///
/// #[derive(clap::Parser, Debug, PartialEq, Eq)]
/// struct Run {
///     #[arg(long)]
///     name: String,
/// }
///
/// let opts = Command::<Run>::parse_from(["foobar-operator", "run", "--name", "foo"]);
/// assert_eq!(opts, Command::Run(Run {
///     name: "foo".to_string(),
/// }));
/// ```
#[derive(clap::Parser, Debug, PartialEq, Eq)]
#[command(long_about = "")]
pub struct ProductOperatorRun {
    #[command(flatten)]
    pub common: CommonOptions,

    #[command(flatten)]
    pub operator_environment: OperatorEnvironmentOptions,

    /// Provides the path to a product-config file
    #[arg(long, short = 'p', value_name = "FILE", default_value = "", env)]
    pub product_config: ProductConfigPath,

    /// Provides a specific namespace to watch (instead of watching all namespaces)
    #[arg(long, env, default_value = "")]
    pub watch_namespace: WatchNamespace,
}

/// All the CLI arguments that all (or at least most) Stackable applications use.
///
/// [`ProductOperatorRun`] is intended for operators, but it has fields that are not needed for
/// utilities such as `user-info-fetcher` or `opa-bundle-builder`. So this struct offers a limited
/// set, that should be shared across all Stackable tools running on Kubernetes.
#[derive(clap::Parser, Debug, PartialEq, Eq)]
pub struct CommonOptions {
    #[command(flatten)]
    pub telemetry: TelemetryOptions,

    #[command(flatten)]
    pub cluster_info: KubernetesClusterInfoOptions,
}

/// A path to a [`ProductConfigManager`] spec file
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductConfigPath {
    path: Option<PathBuf>,
}

impl From<&OsStr> for ProductConfigPath {
    fn from(s: &OsStr) -> Self {
        Self {
            // StructOpt doesn't let us hook in to see the underlying `Option<&str>`, so we treat the
            // otherwise-invalid `""` as a sentinel for using the default instead.
            path: if s.is_empty() { None } else { Some(s.into()) },
        }
    }
}

impl ProductConfigPath {
    /// Load the [`ProductConfigManager`] from the given path, falling back to the first
    /// path that exists from `default_search_paths` if none is given by the user.
    pub fn load(&self, default_search_paths: &[impl AsRef<Path>]) -> Result<ProductConfigManager> {
        let resolved_path = Self::resolve_path(self.path.as_deref(), default_search_paths)?;
        ProductConfigManager::from_yaml_file(resolved_path).context(ProductConfigLoadSnafu)
    }

    /// Check if the path can be found anywhere
    ///
    /// 1. User provides path `user_provided_path` to file. Return [`Error`] if not existing.
    /// 2. User does not provide path to file -> search in `default_paths` and
    ///    take the first existing file.
    /// 3. Return [`Error`] if nothing was found.
    fn resolve_path<'a>(
        user_provided_path: Option<&'a Path>,
        default_paths: &'a [impl AsRef<Path> + 'a],
    ) -> Result<&'a Path> {
        // Use override if specified by the user, otherwise search through defaults given
        let search_paths = if let Some(path) = user_provided_path {
            vec![path]
        } else {
            default_paths.iter().map(|path| path.as_ref()).collect()
        };
        for path in &search_paths {
            if path.exists() {
                return Ok(path);
            }
        }
        RequiredFileMissingSnafu {
            search_path: search_paths
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
        }
        .fail()
    }
}

#[derive(clap::Parser, Debug, PartialEq, Eq)]
pub struct OperatorEnvironmentOptions {
    /// The namespace the operator is running in, usually `stackable-operators`.
    ///
    /// Note that when running the operator on Kubernetes we recommend to use the
    /// [downward API](https://kubernetes.io/docs/concepts/workloads/pods/downward-api/)
    /// to let Kubernetes project the namespace as the `OPERATOR_NAMESPACE` env variable.
    #[arg(long, env)]
    pub operator_namespace: String,

    /// The name of the service the operator is reachable at, usually
    /// something like `<product>-operator`.
    #[arg(long, env)]
    pub operator_service_name: String,
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use rstest::*;
    use tempfile::tempdir;

    use super::*;

    const USER_PROVIDED_PATH: &str = "user_provided_path_properties.yaml";
    const DEPLOY_FILE_PATH: &str = "deploy_config_spec_properties.yaml";
    const DEFAULT_FILE_PATH: &str = "default_file_path_properties.yaml";

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        ProductOperatorRun::command().print_long_help().unwrap();
        ProductOperatorRun::command().debug_assert()
    }

    #[rstest]
    #[case(
        Some(USER_PROVIDED_PATH),
        vec![],
        USER_PROVIDED_PATH,
        USER_PROVIDED_PATH
    )]
    #[case(
        None,
        vec![DEPLOY_FILE_PATH, DEFAULT_FILE_PATH],
        DEPLOY_FILE_PATH,
        DEPLOY_FILE_PATH
    )]
    #[case(None, vec!["bad", DEFAULT_FILE_PATH], DEFAULT_FILE_PATH, DEFAULT_FILE_PATH)]
    fn resolve_path_good(
        #[case] user_provided_path: Option<&str>,
        #[case] default_locations: Vec<&str>,
        #[case] path_to_create: &str,
        #[case] expected: &str,
    ) -> Result<()> {
        let temp_dir = tempdir().expect("create temporary directory");
        let full_path_to_create = temp_dir.path().join(path_to_create);
        let full_user_provided_path = user_provided_path.map(|p| temp_dir.path().join(p));
        let expected_path = temp_dir.path().join(expected);

        let mut full_default_locations = vec![];

        for loc in default_locations {
            let temp = temp_dir.path().join(loc);
            full_default_locations.push(temp.as_path().display().to_string());
        }

        let full_default_locations_ref = full_default_locations
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        let file = File::create(full_path_to_create).expect("create temporary file");

        let found_path = ProductConfigPath::resolve_path(
            full_user_provided_path.as_deref(),
            &full_default_locations_ref,
        )?;

        assert_eq!(found_path, expected_path);

        drop(file);
        temp_dir.close().expect("clean up temporary directory");

        Ok(())
    }

    #[test]
    #[should_panic]
    fn resolve_path_user_path_not_existing() {
        ProductConfigPath::resolve_path(Some(USER_PROVIDED_PATH.as_ref()), &[DEPLOY_FILE_PATH])
            .unwrap();
    }

    #[test]
    fn resolve_path_nothing_found_errors() {
        if let Err(Error::RequiredFileMissing { search_path }) =
            ProductConfigPath::resolve_path(None, &[DEPLOY_FILE_PATH, DEFAULT_FILE_PATH])
        {
            assert_eq!(
                search_path,
                vec![
                    PathBuf::from(DEPLOY_FILE_PATH),
                    PathBuf::from(DEFAULT_FILE_PATH)
                ]
            )
        } else {
            panic!("must return RequiredFileMissing when file was not found")
        }
    }
}
