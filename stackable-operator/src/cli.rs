use crate::error;
use crate::error::OperatorResult;
use clap::{App, Arg};
use std::path::Path;

/// Retrieve a file path from CLI arguments that points to product-config file.
/// It is a temporary solution until we find out how to handle different CLI
/// arguments for different operators.
// TODO: write proper init method for all possible operator-rs arguments plus
//    operator specific arguments
pub fn product_config_path(name: &str, default_locations: Vec<&str>) -> OperatorResult<String> {
    let argument = "product-config";

    let matches = App::new(name)
        .arg(
            Arg::with_name(argument)
                .short("p")
                .long(argument)
                .value_name("FILE")
                .help("Get path to a product-config file")
                .takes_value(true),
        )
        .get_matches();

    check_path(matches.value_of(argument), default_locations)
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
            search_paths.push(path.to_string());
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
            search_paths.push(loc.to_string())
        }
    }

    // 3) Error if nothing was found
    Err(error::Error::RequiredFileMissing {
        search_path: search_paths,
    })
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
        if let Err(error::Error::RequiredFileMissing {
            search_path: errors,
        }) = check_path(None, vec![DEPLOY_FILE_PATH, DEFAULT_FILE_PATH])
        {
            assert_eq!(errors, vec![DEPLOY_FILE_PATH, DEFAULT_FILE_PATH])
        }
    }
}
