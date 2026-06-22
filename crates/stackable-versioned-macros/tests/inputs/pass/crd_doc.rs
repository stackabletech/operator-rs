use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"))]
// ---
pub(crate) mod versioned {
    #[versioned(crd(
        group = "stackable.tech",
        doc = "A FooCluster, deployed and managed by the example operator."
    ))]
    #[derive(
        Clone,
        Debug,
        serde::Deserialize,
        serde::Serialize,
        schemars::JsonSchema,
        kube::CustomResource,
    )]
    pub(crate) struct FooSpec {}
}
// ---
fn main() {}
