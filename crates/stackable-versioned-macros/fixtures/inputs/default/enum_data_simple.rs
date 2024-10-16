#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
enum Foo {
    Foo,
    Bar(u32, String),

    #[versioned(added(since = "v1alpha2"))]
    Baz {
        id: u32,
        name: String,
    },
}
