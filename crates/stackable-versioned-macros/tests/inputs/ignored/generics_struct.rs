use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1"))]
// ---
pub struct Foo<T>
where
    T: Default,
{
    bar: T,
    baz: u8,
}
// ---
fn main() {}
