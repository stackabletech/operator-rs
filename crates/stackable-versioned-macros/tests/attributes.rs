use stackable_versioned_macros::versioned;

#[ignore]
#[test]
fn pass_struct_attributes() {
    /// General struct docs that cover all versions.
    #[versioned(
        version(name = "v1alpha1"),
        version(
            name = "v1beta1",
            doc = r#"
                Additional docs for this version which are purposefully long to
                show how manual line wrapping works. \
                Multi-line docs are also supported, as per regular doc-comments.
            "#
        ),
        version(name = "v1beta2"),
        version(name = "v1"),
        version(name = "v2"),
        options(skip(from))
    )]
    #[derive(Default)]
    struct Foo {
        /// This field is available in every version (so far).
        foo: String,

        /// Keep the main field docs the same, even after the field is deprecated.
        #[versioned(deprecated(since = "v1beta1", note = "gone"))]
        deprecated_bar: String,

        /// This is for baz
        #[versioned(added(since = "v1beta1"))]
        baz: String,

        /// This is will keep changing over time.
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

#[ignore]
#[allow(dead_code)]
#[test]
fn pass_enum_attributes() {
    /// General enum docs that cover all versions.
    #[versioned(
        version(name = "v1alpha1"),
        version(
            name = "v1beta1",
            doc = r#"
                Additional docs for this version which are purposefully long to
                show how manual line wrapping works. \
                Multi-line docs are also supported, as per regular doc-comments.
            "#
        ),
        version(name = "v1beta2"),
        version(name = "v1"),
        version(name = "v2"),
        options(skip(from))
    )]
    #[derive(Default)]
    enum Foo {
        /// This variant is available in every version (so far).
        #[default]
        Foo,

        /// Keep the main field docs the same, even after the field is
        /// deprecated.
        #[versioned(deprecated(since = "v1beta1", note = "gone"))]
        DeprecatedBar,

        /// This is for baz
        #[versioned(added(since = "v1beta1"))]
        // Just to check stackable-versioned deprecation warning appears.
        // #[deprecated]
        Baz,

        /// This is will keep changing over time.
        #[versioned(renamed(since = "v1beta1", from = "Qoox"))]
        #[versioned(renamed(since = "v1", from = "Qaax"))]
        Quux,
    }
}
