use stackable_versioned_macros::versioned;

#[allow(dead_code)]
fn main() {
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
        #[versioned(changed(since = "v1beta1", from_name = "Qoox"))]
        #[versioned(changed(since = "v1", from_name = "Qaax"))]
        Quux,
    }
}
