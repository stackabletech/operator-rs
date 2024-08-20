use stackable_versioned_macros::versioned;

#[ignore]
#[test]
fn pass_container_attributes() {
    /// General struct docs that cover all versions.
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1beta2"),
        version(name = "v1"),
        version(name = "v2"),
        options(skip(from)),
    )]
    struct Foo {
        foo: String,

        #[versioned(deprecated(since = "v1beta1", note = "gone"))]
        deprecated_bar: String,

        #[versioned(added(since = "v1beta1"))]
        baz: String,

        #[versioned(renamed(since = "v1beta1", from = "qoox"))]
        #[versioned(renamed(since = "v1", from = "qaax"))]
        quux: String,
    }

    let _ = v1alpha1::Foo {
        foo: String::from("foo"),
        bar: String::from("Hello"),
        qoox: String::from("world"),
    };

    #[allow(deprecated)]
    let _ = v1beta1::Foo {
        foo: String::from("foo"),
        deprecated_bar: String::from("Hello"),
        baz: String::from("Hello"),
        qaax: String::from("World"),
    };

    #[allow(deprecated)]
    let _ = v1::Foo {
        foo: String::from("foo"),
        deprecated_bar: String::from("Hello"),
        baz: String::from("Hello"),
        quux: String::from("World"),
    };
}
