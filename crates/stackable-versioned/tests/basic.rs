use stackable_versioned::Versioned;

#[derive(Versioned)]
#[allow(dead_code)]
#[versioned(version(name = "v1alpha1", deprecated))]
struct Foo {
    #[versioned(added(since = "v1alpha1"))]
    bar: usize,
}

#[test]
fn basic() {
    // let _ = v1alpha1::Foo {};
    // let _ = latest::Foo {};
}
