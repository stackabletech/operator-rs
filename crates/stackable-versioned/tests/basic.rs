use stackable_versioned::Versioned;

#[derive(Versioned)]
#[allow(dead_code)]
#[versioned(version(name = "v1alpha1"), version(name = "v1beta1"))]
struct Foo {
    #[versioned(deprecated(since = "v1beta1", note = "was moved to some other field"))]
    bar: usize,
    baz: bool,
}

#[test]
fn basic() {
    // let _ = v1alpha1::Foo {};
}
