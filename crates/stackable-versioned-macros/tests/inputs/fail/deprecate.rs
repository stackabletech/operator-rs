use stackable_versioned_macros::versioned;

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
mod versioned {
    struct Foo {
        #[deprecated]
        bar: usize,
        baz: bool,
    }
}

fn main() {}
