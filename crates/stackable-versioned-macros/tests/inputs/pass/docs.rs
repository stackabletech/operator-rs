use stackable_versioned::versioned;
// ---
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
    options(k8s(experimental_conversion_tracking))
)]
// ---
mod versioned {
    /// Test
    #[derive(Default, serde::Deserialize)]
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
        #[versioned(changed(since = "v1beta1", previous_name = "qoox"))]
        #[versioned(changed(since = "v1", previous_name = "qaax"))]
        quux: String,

        /// The docs of this field changed in v1beta1 and v2.
        #[versioned(
            changed(since = "v1beta1", previous_docs = "These are the docs in v1alpha1."),
            changed(
                since = "v2",
                previous_docs = r#"
                    These are the docs from v1beta1 until v1.

                    Multi-line docs are also supported.
                "#
            )
        )]
        // Those must be preserved.
        #[doc(alias = "grault")]
        #[serde(rename = "corge")]
        corge: String,

        /// The docs of this field changed in v1, while it was renamed in v1beta1.
        #[versioned(
            changed(since = "v1beta1", previous_name = "waldo"),
            changed(since = "v1", previous_docs = "These are the docs before v1.")
        )]
        fred: String,
    }

    /// Test
    #[derive(Default)]
    enum Bar {
        /// The docs of this variant changed in v1.
        #[versioned(changed(since = "v1", previous_docs = "These are the docs before v1."))]
        #[default]
        Baz,
    }
}
// ---
fn main() {}
