use stackable_versioned_macros::versioned;

#[ignore]
#[test]
fn pass_container_attributes() {
    /// General docs that cover all versions
    #[versioned(
        version(name = "v1alpha1"),
        version(
            name = "v1beta1",
            doc = r#"
                Additional docs for this version. \
                Supports multi-line docs.
            "#
        )
    )]
    // FIXME(@NickLarsenNZ): Derives
    // #[derive(Default)]
    struct Foo {
        /// Always here
        foo: String,

        /// This is for bar (now deprecated)
        #[versioned(deprecated(since = "v1beta1", note = "gone"))]
        deprecated_bar: String,

        /// This is for baz
        #[versioned(added(since = "v1beta1"))]
        // #[deprecated]
        baz: String,

        /// This is for qaax (previously qoox)
        #[versioned(renamed(since = "v1beta1", from = "qoox"))]
        qaax: String,
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
}
