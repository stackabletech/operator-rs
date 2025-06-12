use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(
        group = "foo.example.org",
        crates(
            kube_core = ::kube::core,
            schemars = ::schemars
        )
    )
)]
// ---
#[derive(
    Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema, kube::CustomResource,
)]
pub struct FooSpec {
    #[versioned(added(since = "v1beta1"), changed(since = "v1", from_name = "bah"))]
    bar: usize,
    baz: bool,
}
// ---
fn main() {}
