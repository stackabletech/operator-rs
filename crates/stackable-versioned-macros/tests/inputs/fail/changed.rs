use stackable_versioned_macros::versioned;

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2")
)]
mod versioned {
    struct Foo {
        #[versioned(
            changed(in = "v1beta1", from_name = "deprecated_bar"),
            changed(in = "v1", from_name = "deprecated_baz"),
            changed(in = "v2")
        )]
        bar: usize,
    }
}

fn main() {}
