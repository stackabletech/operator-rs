use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    k8s(group = "stackable.tech", shortname = "f", shortname = "fo",)
)]
// ---
#[derive(
    Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema, kube::CustomResource,
)]
pub(crate) struct FooSpec {}
// ---
fn main() {}
