use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1"), version(name = "v2"))]
// ---
struct Foo {
    #[versioned(
        // This tests two additional things:
        // - that both unquoted and quoted usage works
        // - that the renamed name does get picked up correctly by the conversion function
        changed(since = "v1", from_type = "u16", from_name = "bar", downgrade_with = u32_to_u16),
        changed(since = "v2", from_type = "u32", downgrade_with = "u64_to_u32")
    )]
    baz: u64,
}
// ---
fn main() {}

fn u32_to_u16(input: u32) -> u16 {
    input.try_into().unwrap()
}

fn u64_to_u32(input: u64) -> u32 {
    input.try_into().unwrap()
}
