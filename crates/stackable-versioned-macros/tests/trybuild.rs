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
    mod default {
        // mod fail {
        //     mod deprecate;
        //     mod skip_from_all;
        //     mod skip_from_version;
        // }
    }

    #[cfg(feature = "k8s")]
    mod k8s {
        // mod fail {
        //     mod crd;
        // }
    }
}

#[test]
fn default() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/comp_inputs/default/fail/*.rs");
    t.pass("tests/comp_inputs/default/pass/*.rs");
}

#[cfg(feature = "k8s")]
#[test]
fn k8s() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/comp_inputs/k8s/fail/*.rs");
}
