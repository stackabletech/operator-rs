use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1beta1"))]
// ---
pub(crate) mod versioned {
    #[versioned(crd(
        group = "stackable.tech",
        selectable = ".spec.bar",
        selectable = ".spec.baz",
        doc = "Test"
    ))]
    #[derive(
        Clone,
        Debug,
        serde::Deserialize,
        serde::Serialize,
        schemars::JsonSchema,
        kube::CustomResource,
    )]
    pub(crate) struct FooSpec {
        bar: String,
        baz: bool,
    }
}
// ---
fn main() {}
