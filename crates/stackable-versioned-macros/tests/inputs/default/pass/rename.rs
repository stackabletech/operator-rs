use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
// ---
struct Foo {
    #[versioned(changed(since = "v1beta1", from_name = "bat"))]
    bar: usize,
    baz: bool,
}
// ---
fn main() {}
