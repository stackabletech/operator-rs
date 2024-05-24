use stackable_versioned::versioned;

// To expand the generated code (for debugging and testing), it is recommended
// to first change directory via `cd crates/stackable-versioned` and to then
// run `cargo expand --test basic --all-features`.
#[allow(dead_code)]
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2"),
    version(name = "v3")
)]
struct Foo {
    /// My docs
    #[versioned(
        added(since = "v1alpha1"),
        renamed(since = "v1beta1", from = "jjj"),
        deprecated(since = "v2", note = "not empty")
    )]
    deprecated_bar: usize,
    baz: bool,
}

#[test]
fn basic() {
    let _ = foo::V1Alpha1 { jjj: 0, baz: false };
    let _ = foo::V1Beta1 { bar: 0, baz: false };
    let _ = foo::V1 { bar: 0, baz: false };

    #[allow(deprecated)]
    let _ = foo::V2 {
        deprecated_bar: 0,
        baz: false,
    };

    // The latest version (v3)
    #[allow(deprecated)]
    let _ = Foo {
        deprecated_bar: 0,
        baz: false,
    };
}
