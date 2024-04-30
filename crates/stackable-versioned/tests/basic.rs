use stackable_versioned::Versioned;

#[derive(Versioned)]
#[allow(dead_code)]
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1", deprecated),
    version(name = "v1beta2"),
    version(name = "v1"),
    version(name = "v2alpha1"),
    version(name = "v2")
)]
struct Foo {
    /// My docs
    #[versioned(added(since = "v1beta1"), renamed(since = "v1beta2", from = "jjj"))]
    bar: usize,
    baz: bool,
}

#[test]
fn basic() {
    // let _foo = v1beta1::Foo { bar: 0, baz: true };
}
