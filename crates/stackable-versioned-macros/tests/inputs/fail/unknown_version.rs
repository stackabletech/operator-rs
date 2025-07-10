use stackable_versioned_macros::versioned;

#[versioned(version(name = "v1alpha1"))]
mod versioned {
    struct Foo {
        #[versioned(added(since = "v1alpha2"))]
        bar: usize,
    }
}

fn main() {}
