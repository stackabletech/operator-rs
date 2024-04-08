use stackable_versioned::Versioned;

#[test]
fn basic() {
    #[derive(Versioned)]
    #[versioned(version(name = "1.2.3", deprecated))]
    struct Foo {
        bar: usize,
    }
}
