use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1"))]
// ---
pub enum Foo<T>
where
    T: Default,
{
    Bar(T),
    Baz,
}
// ---
fn main() {}
