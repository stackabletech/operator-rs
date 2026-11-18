use stackable_versioned_macros::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1alpha2"),
    version(name = "v1beta1"),
    version(name = "v1"),
    version(name = "v2")
)]
// ---
mod versioned {
    struct Foo {
        username: String,

        /// Removed without being deprecated first.
        #[versioned(removed(in = "v1alpha2"))]
        nickname: String,

        #[versioned(
            deprecated(in = "v1alpha2"),
            removed(in = "v1beta1", default = default_age)
        )]
        deprecated_age: u32,

        #[versioned(
            changed(
                in = "v1alpha2",
                previous_name = "colour",
                previous_type = "u8",
                downgrade_with = u16_to_u8
            ),
            removed(in = "v1")
        )]
        color: u16,

        #[versioned(added(in = "v1alpha2"), removed(in = "v1"))]
        email: String,
    }
}
// ---
fn main() {}

fn default_age() -> u32 {
    42
}

fn u16_to_u8(input: u16) -> u8 {
    input.try_into().unwrap()
}
