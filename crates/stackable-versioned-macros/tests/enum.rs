use stackable_versioned_macros::versioned;

#[test]
fn versioned_enum() {
    #[versioned(version(name = "v1alpha1"), version(name = "v1beta1"))]
    pub enum Foo {
        Bar,
        Baz,
    }
}
