use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1"),
        version(name = "v2"),
        version(name = "v3")
    )]
    struct Foo {
        #[versioned(deprecated(since = "v1", note = "gone"))]
        deprecated_bar: usize,
        baz: bool,
    }
}
