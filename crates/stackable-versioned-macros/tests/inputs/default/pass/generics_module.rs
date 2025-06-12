use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1"),
    options(preserve_module)
)]
// ---
pub mod versioned {
    struct Foo<T>
    where
        T: Default,
    {
        bar: T,
        baz: u8,
    }

    enum Boom<T>
    where
        T: Default,
    {
        Big(T),
        Shaq,
    }
}
// ---
fn main() {}
