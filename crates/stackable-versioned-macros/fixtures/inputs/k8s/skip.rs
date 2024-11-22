#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    k8s(
        group = "stackable.tech",
        singular = "foo",
        plural = "foos",
        namespaced,
        skip(merged_crd)
    )
)]
// ---
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct FooSpec {
    #[versioned(
        added(since = "v1beta1"),
        changed(since = "v1", from_name = "bah", from_type = "u16")
    )]
    bar: usize,
    baz: bool,
}
