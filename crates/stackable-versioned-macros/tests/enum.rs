use stackable_versioned_macros::versioned;

#[test]
fn versioned_enum() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1")
    )]
    pub enum Foo {
        #[versioned(added(since = "v1beta1"), deprecated(since = "v1", note = "bye"))]
        DeprecatedBar,
        Baz,
    }
}
