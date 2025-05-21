use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1", skip(from)),
    version(name = "v1")
)]
// ---
pub(crate) mod versioned {
    pub struct Foo {
        bar: usize,

        #[versioned(added(since = "v1beta1"))]
        baz: bool,
    }

    pub struct Bar {
        foo: usize,

        #[versioned(added(since = "v1beta1"))]
        faz: bool,
    }
}
// ---
fn main() {}
