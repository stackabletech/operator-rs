use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1"), version(name = "v2"))]
// ---
struct Foo {
    #[versioned(
        // This tests two additional things:
        // - that both unquoted and quoted usage works
        // - that the renamed name does get picked up correctly by the conversion function
        changed(since = "v1", from_type = "u16", from_name = "bar", upgrade_with = u16_to_u32),
        changed(since = "v2", from_type = "u32", upgrade_with = "u32_to_u64")
    )]
    baz: u64,
}
// ---
fn main() {}

fn u16_to_u32(input: u16) -> u32 {
    input as u32
}

fn u32_to_u64(input: u32) -> u64 {
    input as u64
}
