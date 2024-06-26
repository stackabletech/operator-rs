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
        /// This is for bar
        #[versioned(deprecated(since = "v1beta1", note = "gone"))]
        deprecated_bar: String,

        /// This is for baz
        #[versioned(added(since = "v1beta1"))]
        // #[deprecated]
        baz: String,
    }

    let _ = v1alpha1::Foo {
        bar: String::from("Hello"),
    };

    #[allow(deprecated)]
    let _ = v1beta1::Foo {
        deprecated_bar: String::from("Hello"),
        baz: String::from("Hello"),
    };
}
