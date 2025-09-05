use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
mod versioned {
    struct Foo {
        #[versioned(hint(option))]
        bar: Option<String>,

        #[versioned(hint(vec))]
        baz: Vec<usize>,

        quux: bool,
    }
}
// ---
fn main() {}
