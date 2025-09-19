use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use product_config::ProductConfigManager;
use snafu::{ResultExt, Snafu};

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

#[cfg(test)]
mod tests {
    use std::fs::File;

    use rstest::*;
    use tempfile::tempdir;

    use super::*;

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
