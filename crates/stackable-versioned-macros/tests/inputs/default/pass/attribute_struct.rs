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
    options(skip(from))
)]
// ---
/// Test
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
    #[versioned(changed(since = "v1beta1", from_name = "qoox"))]
    #[versioned(changed(since = "v1", from_name = "qaax"))]
    quux: String,
}
// ---
fn main() {}
