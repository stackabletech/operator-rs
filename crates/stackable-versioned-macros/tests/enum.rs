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

    // There is only one variant in v1alpha1, so we can take a shortcut and thus
    // don't need a match statement
    // impl From<v1alpha1::Foo> for v1beta1::Foo {
    //     fn from(__sv_value: v1alpha1::Foo) -> Self {
    //         Self::Baz
    //     }
    // }

    // We need to match, to do the proper conversion
    // impl From<v1beta1::Foo> for v1::Foo {
    //     fn from(__sv_value: v1beta1::Foo) -> Self {
    //         match __sv_value {
    //             v1beta1::Foo::Bar => Self::DeprecatedBar,
    //             v1beta1::Foo::Baz => Self::Baz,
    //         }
    //     }
    // }
}
