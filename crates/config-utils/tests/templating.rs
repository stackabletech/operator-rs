use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use rstest::rstest;

use config_utils::templating::template;
use tempfile::tempdir;

#[rstest]
fn test_file_templating(#[files("tests/resources/**/*.in")] test_file_in: PathBuf) {
    let example_dir = create_example_files();
    set_example_envs(&example_dir);

    let test_file = test_file_in.with_extension("");
    let test_file_expected = test_file_in.with_extension("expected");

    fs::copy(&test_file_in, &test_file).unwrap();
    template(&test_file, None, true).unwrap();

    let actual = fs::read_to_string(&test_file).unwrap();
    let expected = fs::read_to_string(&test_file_expected).unwrap();

    similar_asserts::assert_eq!(actual, expected);
}

fn set_example_envs(example_dir: &PathBuf) {
    // SAFETY: We only use a single thread to set this env vars
    env::set_var("ENV_TEST", "foo");
    env::set_var("ENV_TEST_USERNAME", "example user");
    env::set_var("ENV_TEST_PASSWORD", "admin-pw= withSpace$%\"' &&} §");
    env::set_var("ENV_TEST_PASSWORD_ENV_NAME", "ENV_TEST_PASSWORD");

    env::set_var("FILE_TEST_42_FILE", example_dir.join("42"));
    env::set_var("FILE_TEST_42_FILE_ENV_NAME", "FILE_TEST_42_FILE");
}

/// Returns the directoy where the files reside
fn create_example_files() -> PathBuf {
    let dir = tempdir().expect("Failed to create temp dir").into_path();

    let mut file = File::create(dir.join("42")).unwrap();
    file.write_all(b"42").unwrap();

    dir
}
