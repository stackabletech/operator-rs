use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(
        group = "stackable.tech",
        status = FooStatus,
        options(experimental_conversion_tracking),
    )
)]
// ---
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
pub(crate) struct FooSpec {
    #[versioned(added(since = "v1beta1"), changed(since = "v1", from_name = "bah"))]
    bar: usize,
    baz: bool,
}
// ---
fn main() {}

#[derive(Clone, Debug, JsonSchema, Deserialize, Serialize)]
pub struct FooStatus {
    foo: String,
}

fn usize_to_u16(input: usize) -> u16 {
    input.try_into().unwrap()
}
