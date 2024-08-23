use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1", skip(from)),
        version(name = "v1")
    )]
    pub struct Foo {
        #[versioned(
            added(since = "v1beta1"),
            deprecated(since = "v1", note = "not needed")
        )]
        deprecated_bar: usize,
        baz: bool,
    }

    let foo_v1alpha1 = v1alpha1::Foo { baz: true };
    let foo_v1beta1 = v1beta1::Foo::from(foo_v1alpha1);

    #[allow(dead_code)]
    // v1beta1 has no From impl. You need to convert it manually.
    let foo_v1 = v1::Foo::from(foo_v1beta1);
}
