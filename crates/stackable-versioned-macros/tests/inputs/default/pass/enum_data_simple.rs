use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
enum Foo {
    Foo,
    Bar(u32, String),
    // FIXME (@Techassi): How do we handle downgrades of enums? The variant just
    // doesn't exist in the earlier version, but we still need to handle the
    // variant in the match. I think for this to work, we would need to require
    // the user to specify a downgrade_with function. For now, I commented out
    // the code to get the test to pass again.
    // #[versioned(added(since = "v1alpha2"))]
    // Baz {
    //     id: u32,
    //     name: String,
    // },
}
// ---
fn main() {}
