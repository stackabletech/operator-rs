use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1", deprecated),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2"),
    version(name = "v3")
)]
// ---
pub(crate) struct Foo {
    #[versioned(added(since = "v1"))]
    foo: String,

    #[versioned(
        changed(since = "v1beta1", from_name = "jjj", from_type = "u8", downgrade_with = u16_to_u8),
        changed(since = "v1", from_type = "u16", downgrade_with = usize_to_u16),
        deprecated(since = "v2", note = "not empty")
    )]
    /// Test
    deprecated_bar: usize,
    baz: bool,
}
// ---
fn main() {}

fn usize_to_u16(input: usize) -> u16 {
    input.try_into().unwrap()
}

fn u16_to_u8(input: u16) -> u8 {
    input.try_into().unwrap()
}
