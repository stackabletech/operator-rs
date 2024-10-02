use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};

use insta::Settings;
use proc_macro2::TokenStream;
use regex::Regex;
use snafu::{OptionExt, ResultExt, Snafu};
use syn::DeriveInput;

use crate::versioned_impl;

const DELIMITER: &str = "// ---\n";

static REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#\[versioned\(\n(?P<args>[[:ascii:]]+)\n\)\]")
        .expect("failed to compile versioned regex")
});

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("failed to read input file"))]
    ReadFile { source: std::io::Error },

    #[snafu(display("failed to find delimiter"))]
    MissingDelimiter,

    #[snafu(display("failed to find regex match group"))]
    MissingRegexMatchGroup,

    #[snafu(display("failed to parse token stream"))]
    ParseTokenStream { source: proc_macro2::LexError },

    #[snafu(display("failed to parse derive input"))]
    ParseDeriveInput { source: syn::Error },

    #[snafu(display("failed to parse output file"))]
    ParseOutputFile { source: syn::Error },
}

pub(crate) fn expand_from_file(path: &Path) -> Result<String, Error> {
    let input = std::fs::read_to_string(path).context(ReadFileSnafu)?;
    let (attrs, input) = prepare_from_string(input)?;

    let expanded = versioned_impl(attrs, input).to_string();
    let parsed = syn::parse_file(&expanded).context(ParseOutputFileSnafu)?;

    Ok(prettyplease::unparse(&parsed))
}

fn prepare_from_string(input: String) -> Result<(TokenStream, DeriveInput), Error> {
    let (attrs, input) = input.split_once(DELIMITER).context(MissingDelimiterSnafu)?;

    let attrs = REGEX
        .captures(attrs)
        .unwrap()
        .name("args")
        .context(MissingRegexMatchGroupSnafu)?
        .as_str();

    let attrs = TokenStream::from_str(attrs).context(ParseTokenStreamSnafu)?;
    let input = TokenStream::from_str(input).context(ParseTokenStreamSnafu)?;
    let input = syn::parse2(input).context(ParseDeriveInputSnafu)?;

    Ok((attrs, input))
}

pub(crate) fn set_snapshot_path() -> Settings {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("env var CARGO_MANIFEST_DIR must be set");
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(PathBuf::from(dir).join("fixtures/snapshots"));

    settings
}
