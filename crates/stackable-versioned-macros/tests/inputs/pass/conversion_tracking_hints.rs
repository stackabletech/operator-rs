use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1alpha2"),
    options(k8s(experimental_conversion_tracking))
)]
// ---
mod versioned {
    struct Foo {
        #[versioned(nested, hint(option))]
        bar: Option<Bar>,

        #[versioned(hint(vec))]
        baz: Vec<usize>,

        quux: bool,
    }

    struct Bar {
        bar_bar: String,

        #[versioned(added(since = "v1alpha2"))]
        baz_baz: u8,
    }
}
// ---
fn main() {}
