//! This module provides helper methods to deal with common CLI options using the `clap` crate.
//!
//! In particular it currently supports handling two kinds of options:
//! * CRD handling (printing & saving to a file)
//! * Product config location
//!
//! # Example
//!
//! This example show the usage of the CRD functionality.
//!
//! ```
//! // Handle CLI arguments
//! use clap::{crate_version, SubCommand};
//! use clap::App;
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
//!         SubCommand::with_name("crd")
//!             .subcommand(cli::generate_crd_subcommand::<FooCluster>())
//!             .subcommand(cli::generate_crd_subcommand::<BarCluster>())
//!     )
//!     .get_matches();
//!
//! if let ("crd", Some(subcommand)) = matches.subcommand() {
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
//! use clap::{crate_version, SubCommand};
//! use stackable_operator::cli;
//! use stackable_operator::error::OperatorResult;
//! use clap::App;
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
use crate::CustomResourceExt;
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use product_config::ProductConfigManager;
use std::{
    convert::Infallible,
    path::{Path, PathBuf},
    str::FromStr,
};
use structopt::StructOpt;

pub const AUTHOR: &str = "Stackable GmbH - info@stackable.de";

#[derive(StructOpt)]
pub enum Command {
    /// Print CRD objects
    Crd,
    /// Run operator
    Run {
        /// Provides the path to a product-config file
        #[structopt(long, short = "p", value_name = "FILE", default_value = "")]
        product_config: ProductConfigPath,
    },
}

pub struct ProductConfigPath {
    // Should be Option<PathBuf>, but that depends on https://github.com/stackabletech/product-config/pull/43
    path: Option<String>,
}

impl FromStr for ProductConfigPath {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            // StructOpt doesn't let us hook in to see the underlying `Option<&str>`, so we treat the
            // otherwise-invalid `""` as a sentinel for using the default instead.
            path: if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            },
        })
    }
}

impl ProductConfigPath {
    /// Load the [`ProductConfigManager`] from the given path, falling back to the first
    /// path that exists from `default_search_paths` if none is given by the user.
    pub fn load(
        self,
        // Should be AsRef<Path>, but that depends on https://github.com/stackabletech/product-config/pull/43
        default_search_paths: &[impl AsRef<str>],
    ) -> OperatorResult<ProductConfigManager> {
        // Use override if specified by the user, otherwise search through defaults given
        let search_paths = if let Some(path) = self.path.as_deref() {
            vec![path]
        } else {
            default_search_paths
                .iter()
                .map(|path| path.as_ref())
                .collect()
        };
        for path in &search_paths {
            if <str as AsRef<Path>>::as_ref(path).exists() {
                return ProductConfigManager::from_yaml_file(path)
                    // Fail early if we try and fail to load any files
                    .map_err(|source| error::Error::ProductConfigLoadError { source });
            }
        }
        Err(error::Error::RequiredFileMissing {
            search_path: search_paths.into_iter().map(PathBuf::from).collect(),
        })
    }
}

const PRODUCT_CONFIG_ARG: &str = "product-config";

/// Generates a clap [`Arg`] that can be used to accept the location of a product configuration file.
///
/// Meant to be handled by [`handle_productconfig_arg`].
///
/// See the module level documentation for a complete example.
#[deprecated(note = "use ProductConfigPath (or Command) instead")]
pub fn generate_productconfig_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name(PRODUCT_CONFIG_ARG)
        .short("p")
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
    check_path(matches.value_of(PRODUCT_CONFIG_ARG), default_locations)
}

/// Check if the product-config can be found anywhere:
/// 1) User provides path `user_provided_file_path` to product-config file -> Error if not existing.
/// 2) User does not provide path to product-config-file -> search in default_locations and
///    take the first existing file.
/// 3) Error if nothing was found.
fn check_path(
    user_provided_file_path: Option<&str>,
    default_locations: Vec<&str>,
) -> OperatorResult<String> {
    let mut search_paths = vec![];

    // 1) User provides path to product-config file -> Error if not existing
    if let Some(path) = user_provided_file_path {
        return if Path::new(path).exists() {
            Ok(path.to_string())
        } else {
            search_paths.push(path.into());
            Err(error::Error::RequiredFileMissing {
                search_path: search_paths,
            })
        };
    }

    // 2) User does not provide path to product-config-file -> search in default_locations and
    //    take the first existing file.
    for loc in default_locations {
        if Path::new(loc).exists() {
            return Ok(loc.to_string());
        } else {
            search_paths.push(loc.into())
        }
    }

    // 3) Error if nothing was found
    Err(error::Error::RequiredFileMissing {
        search_path: search_paths,
    })
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
pub fn generate_crd_subcommand<'a, 'b, T>() -> App<'a, 'b>
where
    T: CustomResourceExt,
{
    let kind = T::api_resource().kind;

    SubCommand::with_name(&kind.to_lowercase())
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("print")
                .short("p")
                .long("print")
                .help("Will print the CRD schema in YAML format to stdout"),
        )
        .arg(
            Arg::with_name("save")
                .short("s")
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
    fn test_check_path_good(
        #[case] user_provided_path: Option<&str>,
        #[case] default_locations: Vec<&str>,
        #[case] path_to_create: &str,
        #[case] expected: &str,
    ) -> OperatorResult<()> {
        let temp_dir = tempdir()?;
        let full_path_to_create = temp_dir.path().join(path_to_create);
        let full_user_provided_path =
            user_provided_path.map(|p| temp_dir.path().join(p).to_str().unwrap().to_string());
        let expected_path = temp_dir.path().join(expected);

        let mut full_default_locations = vec![];

        for loc in default_locations {
            let temp = temp_dir.path().join(loc);
            full_default_locations.push(temp.as_path().display().to_string());
        }

        let full_default_locations_ref =
            full_default_locations.iter().map(String::as_str).collect();

        let file = File::create(full_path_to_create)?;

        let found_path = check_path(
            full_user_provided_path.as_deref(),
            full_default_locations_ref,
        )?;

        assert_eq!(&found_path, expected_path.to_str().unwrap());

        drop(file);
        temp_dir.close()?;

        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_check_path_user_path_not_existing() {
        check_path(Some(USER_PROVIDED_PATH), vec![DEPLOY_FILE_PATH]).unwrap();
    }

    #[test]
    fn test_check_path_nothing_found_errors() {
        if let Err(error::Error::RequiredFileMissing { search_path }) =
            check_path(None, vec![DEPLOY_FILE_PATH, DEFAULT_FILE_PATH])
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
