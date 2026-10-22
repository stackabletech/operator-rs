use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
// ---
mod versioned {
    struct Foo {
        #[versioned(changed(in = "v1beta1", from_name = "bat"))]
        bar: usize,
        baz: bool,
    }
}
// ---
fn main() {}
