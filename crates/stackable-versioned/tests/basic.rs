use stackable_versioned::Versioned;

#[derive(Versioned)]
#[allow(dead_code)]
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2alpha1")
)]
struct Foo {
    /// My docs
    #[versioned(
        added(since = "v1alpha1"),
        // renamed(since = "v1beta1", from = "jjj"),
        // renamed(since = "v1", from = "yyy"),
        // deprecated(since = "v2alpha1", _note = "")
    )]
    deprecated_bar: usize,
    baz: bool,
}

#[test]
fn basic() {
    // let _foo = v1beta1::Foo { bar: 0, baz: true };
}
