use std::{
    fs::{self},
    path::PathBuf,
};

use rstest::rstest;

use config_filler::templating::template;

#[rstest]
fn test_file_templating(#[files("tests/resources/**/*.in")] test_file_in: PathBuf) {
    let test_file = test_file_in.with_extension("");
    let test_file_expected = test_file_in.with_extension("expected");

    fs::copy(&test_file_in, &test_file).unwrap();
    template(&test_file, None).unwrap();

    let actual = fs::read_to_string(&test_file).unwrap();
    let expected = fs::read_to_string(&test_file_expected).unwrap();

    similar_asserts::assert_eq!(actual, expected);
}
