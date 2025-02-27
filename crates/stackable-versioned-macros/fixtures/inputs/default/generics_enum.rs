#[versioned(version(name = "v1alpha1"), version(name = "v1"))]
// ---
pub enum Foo<T>
where
    T: Default,
{
    Bar(T),
    Baz,
}
