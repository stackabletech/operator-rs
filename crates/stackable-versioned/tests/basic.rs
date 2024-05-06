use stackable_versioned::Versioned;

#[derive(Versioned)]
#[allow(dead_code)]
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
struct Foo {
    /// My docs
    #[versioned(
        added(since = "v1alpha1"),
        renamed(since = "v1beta1", from = "jjj"),
        deprecated(since = "v1", _note = "")
    )]
    deprecated_bar: usize,
    baz: bool,
}

#[test]
fn basic() {
    let _ = v1alpha1::Foo { jjj: 0, baz: false };
    let _ = v1beta1::Foo { bar: 0, baz: false };
    let _ = Foo {
        deprecated_bar: 0,
        baz: false,
    };
}
