use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"))]
// ---
pub(crate) mod versioned {
    #[versioned(crd(
        group = "stackable.tech",
        scale(
            spec_replicas_path = ".spec.replicas",
            status_replicas_path = ".status.replicas",
            label_selector_path = ".status.selector"
        )
    ))]
    #[derive(
        Clone,
        Debug,
        serde::Deserialize,
        serde::Serialize,
        schemars::JsonSchema,
        kube::CustomResource,
    )]
    struct FooSpec {
        bar: usize,
        baz: bool,
    }
}
// ---
fn main() {}
