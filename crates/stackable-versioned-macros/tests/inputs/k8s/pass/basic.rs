use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(
        group = "stackable.tech",
        singular = "foo",
        plural = "foos",
        status = FooStatus,
        namespaced,
    )
)]
// ---
#[derive(
    Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema, kube::CustomResource,
)]
pub(crate) struct FooSpec {
    #[versioned(
        added(since = "v1beta1"),
        changed(since = "v1", from_name = "bah", from_type = "u16", downgrade_with = usize_to_u16)
    )]
    bar: usize,
    baz: bool,
}
// ---
fn main() {}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct FooStatus {
    is_foo: bool,
}

fn usize_to_u16(input: usize) -> u16 {
    input.try_into().unwrap()
}
