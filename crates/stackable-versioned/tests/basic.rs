use stackable_versioned::Versioned;

#[derive(Versioned)]
#[allow(dead_code)]
#[versioned(version(name = "v1alpha1"), version(name = "v1beta1"))]
struct Foo {
    /// My docs
    #[versioned(added(since = "v1beta1"))]
    bar: usize,
    baz: bool,
}

#[test]
fn basic() {
    let _foo = v1beta1::Foo { bar: 0, baz: true };
}
