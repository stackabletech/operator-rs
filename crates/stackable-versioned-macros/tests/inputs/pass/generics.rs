use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
pub(crate) mod versioned {
    struct FooSpec<Bar, Baz>
    where
        Bar: Default,
        Baz: Clone,
    {
        bar: Bar,
        baz: Baz,
    }
}
// ---
fn main() {}
