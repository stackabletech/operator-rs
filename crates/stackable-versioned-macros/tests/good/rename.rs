use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1")
    )]
    struct Foo {
        #[versioned(renamed(since = "v1beta1", from = "bat"))]
        bar: usize,
        baz: bool,
    }
}
