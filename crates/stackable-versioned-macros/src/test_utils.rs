use std::{path::PathBuf, str::FromStr, sync::LazyLock};

use insta::Settings;
use proc_macro2::TokenStream;
use regex::Regex;
use syn::DeriveInput;

const DELIMITER: &str = "// ---\n";

static REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#\[versioned\(\n(?P<args>[[:ascii:]]+)\n\)\]")
        .expect("failed to compile versioned regex")
});

pub(crate) fn prepare_from_string(input: String) -> (TokenStream, DeriveInput) {
    let (attrs, input) = input
        .split_once(DELIMITER)
        .expect("failed to find delimiter");

    let attrs = REGEX
        .captures(attrs)
        .unwrap()
        .name("args")
        .expect("args match group must be available")
        .as_str();

    let attrs = TokenStream::from_str(attrs).expect("attrs must parse as a token stream");
    let input = TokenStream::from_str(input).expect("input mus parse as a token stream");
    let input = syn::parse2(input).expect("input must parse as derive input");

    (attrs, input)
}

pub(crate) fn set_snapshot_path() -> Settings {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("env var CARGO_MANIFEST_DIR must be set");
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(PathBuf::from(dir).join("fixtures/snapshots"));

    settings
}
