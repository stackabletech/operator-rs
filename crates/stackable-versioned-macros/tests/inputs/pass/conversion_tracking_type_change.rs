use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    options(k8s(experimental_conversion_tracking))
)]
// ---
pub(crate) mod versioned {
    #[versioned(crd(group = "stackable.tech", doc = "Test"))]
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    pub(crate) struct FooSpec {
        // Two consecutive type changes, the second one combined with a rename. Both are tracked.
        #[versioned(
            changed(since = "v1beta1", from_type = "u16", downgrade_with = u32_to_u16),
            changed(
                since = "v1",
                from_name = "bah",
                from_type = "u32",
                downgrade_with = u64_to_u32
            )
        )]
        bar: u64,

        // A rename without a type change is lossless and as such is not tracked.
        #[versioned(changed(since = "v1", from_name = "qux"))]
        baz: bool,

        // An added field in the same version as a type change.
        #[versioned(added(since = "v1"))]
        quux: String,
    }
}
// ---
fn main() {}

fn u32_to_u16(input: u32) -> u16 {
    input.try_into().unwrap_or(u16::MAX)
}

fn u64_to_u32(input: u64) -> u32 {
    input.try_into().unwrap_or(u32::MAX)
}
