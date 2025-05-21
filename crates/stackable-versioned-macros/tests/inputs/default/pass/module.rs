use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1"),
    version(name = "v2alpha1")
)]
// ---
pub(crate) mod versioned {
    pub struct Foo {
        bar: usize,

        #[versioned(added(since = "v1"))]
        baz: bool,

        #[versioned(deprecated(since = "v2alpha1"))]
        deprecated_foo: String,
    }

    // The following attribute is just to ensure no strange behavior occurs.
    #[versioned]
    pub struct Bar {
        baz: String,
    }
}
// ---
fn main() {}
