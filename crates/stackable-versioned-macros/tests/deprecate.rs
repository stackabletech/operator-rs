use stackable_versioned_macros::versioned;

#[ignore]
#[test]
fn deprecate() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1")
    )]
    struct Foo {
        #[versioned(deprecated(since = "v1beta1", note = "gone"))]
        deprecated_bar: usize,
        baz: bool,
    }
}
