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
//! use kube::{CustomResource, CustomResourceExt};
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//! use stackable_operator::cli;
//! use stackable_operator::error::OperatorResult;
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
//! #[clap(
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
//! # fn main() -> OperatorResult<()> {
//! let opts = Opts::from_args();
//!
//! match opts.command {
//!     cli::Command::Crd => println!(
//!         "{}{}",
//!         serde_yaml::to_string(&FooCluster::crd())?,
//!         serde_yaml::to_string(&BarCluster::crd())?,
//!     ),
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
//! use stackable_operator::error::OperatorResult;
//!
//! #[derive(clap::Parser)]
//! #[clap(
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
//! # fn main() -> OperatorResult<()> {
//! let opts = Opts::from_args();
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
use crate::error::OperatorResult;
use crate::namespace::WatchNamespace;
use crate::{error, logging::TracingTarget};
use clap::Args;
use product_config::ProductConfigManager;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub const AUTHOR: &str = "Stackable GmbH - info@stackable.de";

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
#[clap(long_about = "")]
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
/// # use stackable_operator::cli::{Command, ProductOperatorRun, ProductConfigPath};
/// #[derive(clap::Parser, Debug, PartialEq, Eq)]
/// struct Run {
///     #[clap(long)]
///     name: String,
///     #[clap(flatten)]
///     common: ProductOperatorRun,
/// }
/// use clap::Parser;
/// use stackable_operator::logging::TracingTarget;
/// use stackable_operator::namespace::WatchNamespace;
/// let opts = Command::<Run>::parse_from(["foobar-operator", "run", "--name", "foo", "--product-config", "bar", "--watch-namespace", "foobar"]);
/// assert_eq!(opts, Command::Run(Run {
///     name: "foo".to_string(),
///     common: ProductOperatorRun {
///         product_config: ProductConfigPath::from("bar".as_ref()),
///         watch_namespace: WatchNamespace::One("foobar".to_string()),
///         tracing_target: TracingTarget::None
///     },
/// }));
/// ```
///
/// or replaced entirely
///
/// ```rust
/// # use stackable_operator::cli::{Command, ProductOperatorRun};
/// #[derive(clap::Parser, Debug, PartialEq, Eq)]
/// struct Run {
///     #[clap(long)]
///     name: String,
/// }
/// use clap::Parser;
/// let opts = Command::<Run>::parse_from(["foobar-operator", "run", "--name", "foo"]);
/// assert_eq!(opts, Command::Run(Run {
///     name: "foo".to_string(),
/// }));
/// ```
#[derive(clap::Parser, Debug, PartialEq, Eq)]
#[clap(long_about = "")]
pub struct ProductOperatorRun {
    /// Provides the path to a product-config file
    #[clap(
        long,
        short = 'p',
        value_name = "FILE",
        default_value = "",
        env,
        parse(from_os_str)
    )]
    pub product_config: ProductConfigPath,
    /// Provides a specific namespace to watch (instead of watching all namespaces)
    #[clap(long, env, default_value = "", parse(from_str))]
    pub watch_namespace: WatchNamespace,
    /// Tracing log collector system
    #[clap(long, env, default_value_t, arg_enum)]
    pub tracing_target: TracingTarget,
}

/// A path to a [`ProductConfigManager`] spec file
#[derive(Debug, PartialEq, Eq)]
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
    pub fn load(
        &self,
        default_search_paths: &[impl AsRef<Path>],
    ) -> OperatorResult<ProductConfigManager> {
        ProductConfigManager::from_yaml_file(resolve_path(
            self.path.as_deref(),
            default_search_paths,
        )?)
        .map_err(|source| error::Error::ProductConfigLoadError { source })
    }
}

/// Check if the path can be found anywhere:
/// 1) User provides path `user_provided_path` to file -> 'Error' if not existing.
/// 2) User does not provide path to file -> search in `default_paths` and
///    take the first existing file.
/// 3) `Error` if nothing was found.
fn resolve_path<'a>(
    user_provided_path: Option<&'a Path>,
    default_paths: &'a [impl AsRef<Path> + 'a],
) -> OperatorResult<&'a Path> {
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
    Err(error::Error::RequiredFileMissing {
        search_path: search_paths.into_iter().map(PathBuf::from).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::*;
    use std::env;
    use std::fs::File;
    use tempfile::tempdir;

    const USER_PROVIDED_PATH: &str = "user_provided_path_properties.yaml";
    const DEPLOY_FILE_PATH: &str = "deploy_config_spec_properties.yaml";
    const DEFAULT_FILE_PATH: &str = "default_file_path_properties.yaml";
    const WATCH_NAMESPACE: &str = "WATCH_NAMESPACE";

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
    fn test_resolve_path_good(
        #[case] user_provided_path: Option<&str>,
        #[case] default_locations: Vec<&str>,
        #[case] path_to_create: &str,
        #[case] expected: &str,
    ) -> OperatorResult<()> {
        let temp_dir = tempdir()?;
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

        let file = File::create(full_path_to_create)?;

        let found_path = resolve_path(
            full_user_provided_path.as_deref(),
            &full_default_locations_ref,
        )?;

        assert_eq!(found_path, expected_path);

        drop(file);
        temp_dir.close()?;

        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_resolve_path_user_path_not_existing() {
        resolve_path(Some(USER_PROVIDED_PATH.as_ref()), &[DEPLOY_FILE_PATH]).unwrap();
    }

    #[test]
    fn test_resolve_path_nothing_found_errors() {
        if let Err(error::Error::RequiredFileMissing { search_path }) =
            resolve_path(None, &[DEPLOY_FILE_PATH, DEFAULT_FILE_PATH])
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

    #[test]
    fn test_product_operator_run_watch_namespace() {
        // clean env var to not interfere if already set
        env::remove_var(WATCH_NAMESPACE);

        // cli with namespace
        let opts = ProductOperatorRun::parse_from([
            "run",
            "--product-config",
            "bar",
            "--watch-namespace",
            "foo",
        ]);
        assert_eq!(
            opts,
            ProductOperatorRun {
                product_config: ProductConfigPath::from("bar".as_ref()),
                watch_namespace: WatchNamespace::One("foo".to_string()),
                tracing_target: TracingTarget::None,
            }
        );

        // no cli / no env
        let opts = ProductOperatorRun::parse_from(["run", "--product-config", "bar"]);
        assert_eq!(
            opts,
            ProductOperatorRun {
                product_config: ProductConfigPath::from("bar".as_ref()),
                watch_namespace: WatchNamespace::All,
                tracing_target: TracingTarget::None,
            }
        );

        // env with namespace
        env::set_var(WATCH_NAMESPACE, "foo");
        let opts = ProductOperatorRun::parse_from(["run", "--product-config", "bar"]);
        assert_eq!(
            opts,
            ProductOperatorRun {
                product_config: ProductConfigPath::from("bar".as_ref()),
                watch_namespace: WatchNamespace::One("foo".to_string()),
                tracing_target: TracingTarget::None,
            }
        );
    }
}
