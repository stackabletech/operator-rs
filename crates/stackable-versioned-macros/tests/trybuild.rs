//! Add code examples that you expect to compile to tests/good.
//! Add code examples that you expect to fail compilation to tests/bad.
//! Optionally enable/disable the modules below to make local editing easier.
//!
//! Please read the [trybuild workflow][1] docs to understand how to deal with
//! failing test output.
//!
//! [1]: https://github.com/dtolnay/trybuild?tab=readme-ov-file#workflow

// Enable the 'pass' module below to get syntax highlighting and code completion.
// Adjust the list of modules to enable syntax highlighting and code completion.
// Unfortunately tests in sub-folders aren't automatically included.
//
// Similar to the above 'pass' module, enable the 'fail' module below to get
// syntax highlighting and code completion. You will need to comment them out
// again but before running tests, otherwise compilation will fail (as expected).
#[allow(dead_code)]
mod inputs {
    mod pass {
        // mod added;
        // mod basic;
        // mod conversion_hints;
        // mod conversion_tracking_hints;
        // mod conversion_tracking;
        // mod crate_overrides;
        // mod docs;
        // mod downgrade_with;
        // mod enum_fields;
        // mod module;
        // mod module_preserve;
        // mod renamed_field;
        // mod renamed_kind;
        // mod shortnames;
        // mod submodule;
    }

    mod fail {
        // mod applied_to_struct;
        // mod changed;
        // mod deprecate;
        // mod spec_suffix;
        // mod unknown_version;
        // mod submodule_invalid_name;
    }
}

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/inputs/pass/*.rs");
    t.compile_fail("tests/inputs/fail/*.rs");
}
