use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(
        group = "foo.example.org",
        singular = "foo",
        plural = "foos",
        namespaced,
        crates(
            kube_core = ::kube::core
        )
    )
)]
// ---
#[derive(
    Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema, kube::CustomResource,
)]
pub struct FooSpec {
    #[versioned(
        added(since = "v1beta1"),
        changed(since = "v1", from_name = "bah", from_type = "u16")
    )]
    bar: usize,
    baz: bool,
}
// ---
fn main() {}
