use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1")
    )]
    struct Foo {
        #[versioned(
            changed(since = "v1beta1", from_name = "deprecated_bar"),
            changed(since = "v1", from_name = "deprecated_baz")
        )]
        bar: usize,
    }
}
