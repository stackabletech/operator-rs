use stackable_versioned_macros::versioned;

#[allow(deprecated)]
#[test]
fn from() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
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
    let foo_v1 = v1::Foo::from(foo_v1beta1);

    assert_eq!(foo_v1.deprecated_bar, 0);
    assert!(foo_v1.baz);
}

#[test]
fn from_custom_default_fn() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1")
    )]
    pub struct Foo {
        #[versioned(
            added(since = "v1beta1", default = "default_bar"),
            deprecated(since = "v1", note = "not needed")
        )]
        deprecated_bar: usize,
        baz: bool,
    }

    fn default_bar() -> usize {
        42
    }

    let foo_v1alpha1 = v1alpha1::Foo { baz: true };
    let foo_v1beta1 = v1beta1::Foo::from(foo_v1alpha1);

    assert_eq!(foo_v1beta1.bar, 42);
    assert!(foo_v1beta1.baz);
}

#[ignore]
#[test]
fn skip_from_all() {
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
}

#[ignore]
#[test]
fn skip_from_version() {
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
}
