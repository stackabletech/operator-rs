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
//! ```
//! // Handle CLI arguments
//! use clap::{crate_version, App};
//! use stackable_operator::cli;
//! use stackable_operator::error::OperatorResult;
//! use kube::CustomResource;
//! use schemars::JsonSchema;
//! use serde::{Serialize, Deserialize};
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
//! # fn main() -> OperatorResult<()> {
//! let matches = App::new("Spark Operator")
//!     .author("Stackable GmbH - info@stackable.de")
//!     .about("Stackable Operator for Foobar")
//!     .version(crate_version!())
//!     .subcommand(
//!         App::new("crd")
//!             .subcommand(cli::generate_crd_subcommand::<FooCluster>())
//!             .subcommand(cli::generate_crd_subcommand::<BarCluster>())
//!     )
//!     .get_matches();
//!
//! if let Some(("crd", subcommand)) = matches.subcommand() {
//!     if cli::handle_crd_subcommand::<FooCluster>(subcommand)? {
//!         return Ok(());
//!     };
//!     if cli::handle_crd_subcommand::<BarCluster>(subcommand)? {
//!         return Ok(());
//!     };
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Product config handling works similarly:
//!
//! ```no_run
//! use clap::{crate_version, App};
//! use stackable_operator::cli;
//! use stackable_operator::error::OperatorResult;
//!
//! # fn main() -> OperatorResult<()> {
//! let matches = App::new("Spark Operator")
//!     .author("Stackable GmbH - info@stackable.de")
//!     .about("Stackable Operator for Foobar")
//!     .version(crate_version!())
//!     .arg(cli::generate_productconfig_arg())
//!     .get_matches();
//!
//! let paths = vec![
//!     "deploy/config-spec/properties.yaml",
//!     "/etc/stackable/spark-operator/config-spec/properties.yaml",
//! ];
//! let product_config_path = cli::handle_productconfig_arg(&matches, paths)?;
//! # Ok(())
//! # }
//! ```
//!
//!
use crate::error;
use crate::error::OperatorResult;
#[allow(deprecated)]
use crate::CustomResourceExt;
use clap::{App, AppSettings, Arg, ArgMatches};
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
///     #[structopt(flatten)]
///     Framework(stackable_operator::cli::Command)
/// }
/// ```
#[derive(clap::Parser)]
// The enum-level doccomment is intended for developers, not end users
// so supress it from being included in --help
#[clap(long_about = "")]
pub enum Command {
    /// Print CRD objects
    Crd,
    /// Run operator
    Run {
        /// Provides the path to a product-config file
        #[clap(
            long,
            short = 'p',
            value_name = "FILE",
            default_value = "",
            parse(from_os_str)
        )]
        product_config: ProductConfigPath,
    },
}

/// A path to a [`ProductConfigManager`] spec file
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

const PRODUCT_CONFIG_ARG: &str = "product-config";

/// Generates a clap [`Arg`] that can be used to accept the location of a product configuration file.
///
/// Meant to be handled by [`handle_productconfig_arg`].
///
/// See the module level documentation for a complete example.
#[deprecated(note = "use ProductConfigPath (or Command) instead")]
pub fn generate_productconfig_arg() -> Arg<'static> {
    Arg::new(PRODUCT_CONFIG_ARG)
        .short('p')
        .long(PRODUCT_CONFIG_ARG)
        .value_name("FILE")
        .help("Provides the path to a product-config file")
        .takes_value(true)
}

/// Handles the `product-config` CLI option.
///
/// See the module level documentation for a complete example.
///
/// # Arguments
///
/// * `default_locations`: These locations will be checked for the existence of a config file if the user doesn't provide one
#[deprecated(note = "use ProductConfigPath (or Command) instead")]
pub fn handle_productconfig_arg(
    matches: &ArgMatches,
    default_locations: Vec<&str>,
) -> OperatorResult<String> {
    Ok(resolve_path(
        matches.value_of(PRODUCT_CONFIG_ARG).map(str::as_ref),
        &default_locations,
    )?
    .to_str()
    // ArgMatches::value_of and `str` both already validate UTF-8, so this should never be possible
    .expect("product-config path must be UTF-8")
    .to_owned())
}

/// This will generate a clap subcommand ([`App`]) that can be used for operations on CRDs.
///
/// Currently two arguments are supported:
/// * `print`: This will print the schema to stdout
/// * `save`: This will save the schema to a file
///
/// The resulting subcommand can be handled by the [`self::handle_crd_subcommand`] method.
///
/// See the module level documentation for a complete example.
///
/// # Arguments
///
/// * `name`: Name of the CRD
///
/// returns: App
#[deprecated(note = "use Command instead")]
#[allow(deprecated)]
pub fn generate_crd_subcommand<T>() -> App<'static>
where
    T: CustomResourceExt,
{
    let kind = T::api_resource().kind;

    App::new(&kind.to_lowercase())
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("print")
                .short('p')
                .long("print")
                .help("Will print the CRD schema in YAML format to stdout"),
        )
        .arg(
            Arg::with_name("save")
                .short('s')
                .long("save")
                .takes_value(true)
                .value_name("FILE")
                .conflicts_with("print")
                .help("Will write the CRD schema in YAML format to the specified location"),
        )
}

/// This will handle a subcommand generated by the [`self::generate_crd_subcommand`] method.
///
/// The CRD and the name of the subcommand will be identified by the `kind` of the generic parameter `T` being passed in.
///
/// See the module level documentation for a complete example.
///
/// # Arguments
///
/// * `matches`: The [`ArgMatches`] object which _might_ contain a match for our current CRD.
///
/// returns: A boolean wrapped in a result indicating whether the this method did handle the argument.
///          If it returns `Ok(true)` the program should abort.
#[deprecated(note = "use Command instead")]
#[allow(deprecated)]
pub fn handle_crd_subcommand<T>(matches: &ArgMatches) -> OperatorResult<bool>
where
    T: CustomResourceExt,
{
    if let Some(crd_match) = matches.subcommand_matches(T::api_resource().kind.to_lowercase()) {
        if crd_match.is_present("print") {
            T::print_yaml_schema()?;
            return Ok(true);
        }
        if let Some(value) = crd_match.value_of("save") {
            T::write_yaml_schema(value)?;
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::fs::File;
    use tempfile::tempdir;

    const USER_PROVIDED_PATH: &str = "user_provided_path_properties.yaml";
    const DEPLOY_FILE_PATH: &str = "deploy_config_spec_properties.yaml";
    const DEFAULT_FILE_PATH: &str = "default_file_path_properties.yaml";

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
}
