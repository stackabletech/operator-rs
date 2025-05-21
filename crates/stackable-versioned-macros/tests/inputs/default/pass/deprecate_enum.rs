use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2"),
    version(name = "v3")
)]
// ---
enum Foo {
    #[versioned(deprecated(since = "v1"))]
    DeprecatedBar,
    Baz,
}
// ---
fn main() {}
