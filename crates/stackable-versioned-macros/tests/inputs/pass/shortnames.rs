use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"))]
// ---
pub(crate) mod versioned {
    #[versioned(crd(group = "stackable.tech", shortname = "f", shortname = "fo"))]
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
