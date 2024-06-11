use std::{env, fs, path::PathBuf};

use rstest::rstest;

use config_utils::templating::template;

#[rstest]
fn test_file_templating(#[files("tests/resources/**/*.in")] test_file_in: PathBuf) {
    set_example_envs();

    let test_file = test_file_in.with_extension("");
    let test_file_expected = test_file_in.with_extension("expected");

    fs::copy(&test_file_in, &test_file).unwrap();
    template(&test_file, None, true).unwrap();

    let actual = fs::read_to_string(&test_file).unwrap();
    let expected = fs::read_to_string(&test_file_expected).unwrap();

    similar_asserts::assert_eq!(actual, expected);
}

fn set_example_envs() {
    // SAFETY: We only use a single thread to set this env vars
    env::set_var("ENV_TEST", "foo");
    env::set_var("ENV_TEST_PASSWORD", "admin-pw= withSpace$%\"' &&} §");
}
