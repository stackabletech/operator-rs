#[versioned(version(name = "v1alpha1"), version(name = "v1"))]
// ---
pub struct Foo<T = String>
where
    T: Default,
{
    bar: T,
    baz: u8,
}
