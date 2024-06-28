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

    let v1alpha1_foo = v1alpha1::Foo::Baz;
    let v1beta1_foo = v1beta1::Foo::from(v1alpha1_foo);
    let v1_foo = v1::Foo::from(v1beta1_foo);

    // TODO (@Techassi): Forward derive PartialEq
    assert!(matches!(v1_foo, v1::Foo::Baz))
}
