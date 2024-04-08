use stackable_versioned::Versioned;

#[test]
fn basic() {
    #[derive(Versioned)]
    #[allow(dead_code)]
    #[versioned(version(name = "1.2.3", deprecated))]
    struct Foo {
        #[versioned(added(since = "1.2.3"))]
        bar: usize,
    }
}
