use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    options(skip(from))
)]
// ---
pub(crate) mod versioned {
    pub struct Foo {
        bar: usize,

        #[versioned(added(since = "v1beta1"))]
        baz: bool,
    }
}
// ---
fn main() {}
