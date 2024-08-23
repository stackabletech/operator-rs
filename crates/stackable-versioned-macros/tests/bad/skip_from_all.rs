use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1"),
        options(skip(from))
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

    // There are no From impls for any version. You need to convert it manually.
    #[allow(dead_code)]
    let foo_v1beta1 = v1beta1::Foo::from(foo_v1alpha1);
}
