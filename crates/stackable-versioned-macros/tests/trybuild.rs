//! Add code examples that you expect to compile to tests/good.
//! Add code examples that you expect to fail compilation to tests/bad.
//! Optionally enable/disable the modules below to make local editing easier.
//!
//! Please read the [trybuild workflow][1] docs to understand how to deal with
//! failing test output.
//!
//! [1]: https://github.com/dtolnay/trybuild?tab=readme-ov-file#workflow

// Enable the module below to get syntax highlighting and code completion.
// Adjust the list of modules to enable syntax highlighting and code completion.
// Unfortunately tests in subfolders aren't automatically included.
//
// #[allow(dead_code)]
// mod good {
//     mod attributes_enum;
//     mod attributes_struct;
//     mod basic;

//     #[cfg(feature = "k8s")]
//     mod crd;
//     mod deprecate;
//     mod rename;
//     mod skip_from_version;
// }

// Similar to the above module, enable the module below to get syntax
// highlighting and code completion. You will need to comment them out again but
// before running tests, orherwise compilation will fail (as expected).
//
// #[allow(dead_code)]
// mod bad {
//     mod deprecate;
//     mod skip_from_all;
//     mod skip_from_version;
// }

#[test]
fn macros() {
    let t = trybuild::TestCases::new();
    t.pass("tests/good/*.rs");
    t.compile_fail("tests/bad/*.rs");
}
