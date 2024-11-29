#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1", skip(from)),
    version(name = "v1")
)]
// ---
pub struct Foo {
    #[versioned(
        added(since = "v1beta1"),
        deprecated(since = "v1", note = "not needed")
    )]
    deprecated_bar: usize,
    baz: bool,
}
