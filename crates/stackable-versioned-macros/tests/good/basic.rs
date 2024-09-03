use stackable_versioned_macros::versioned;

// To expand the generated code (for debugging and testing), it is recommended
// to first change directory via `cd crates/stackable-versioned` and to then
// run `cargo expand --test basic --all-features`.
#[allow(dead_code)]
#[versioned(
    version(name = "v1alpha1", deprecated),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2"),
    version(name = "v3")
)]
pub(crate) struct Foo {
    #[versioned(
        added(since = "v1alpha1"),
        changed(since = "v1beta1", from_name = "jjj", from_type = "u8"),
        changed(since = "v1", from_type = "u16"),
        deprecated(since = "v2", note = "not empty")
    )]
    /// Test
    deprecated_bar: usize,
    baz: bool,
}

fn main() {
    #[allow(deprecated)]
    let _ = v1alpha1::Foo { jjj: 0, baz: false };
    let _ = v1beta1::Foo { bar: 0, baz: false };
    let _ = v1::Foo { bar: 0, baz: false };

    #[allow(deprecated)]
    let _ = v2::Foo {
        deprecated_bar: 0,
        baz: false,
    };

    // The latest version (v3)
    #[allow(deprecated)]
    let _ = v3::Foo {
        deprecated_bar: 0,
        baz: false,
    };
}
