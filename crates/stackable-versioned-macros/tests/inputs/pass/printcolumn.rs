use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1beta1"))]
// ---
pub(crate) mod versioned {
    #[versioned(crd(
        group = "stackable.tech",
        printcolumn(name = "Bar", json_path = ".spec.bar", type_ = "string"),
        printcolumn(
            name = "Baz",
            json_path = ".spec.baz",
            type_ = "integer",
            description = "The baz value",
            format = "int32",
            priority = 1
        ),
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
        baz: u16,
    }
}
// ---
fn main() {}
