use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned::versioned;
// ---
#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1"),
    options(k8s(experimental_conversion_tracking))
)]
// ---
pub(crate) mod versioned {
    #[versioned(crd(group = "stackable.tech", doc = "Test"))]
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    pub(crate) struct FooSpec {
        // Removed fields are currently not tracked. Their value is lost during upgrades.
        #[versioned(removed(in = "v1beta1"))]
        bar: usize,

        #[versioned(deprecated(in = "v1beta1"), removed(in = "v1"))]
        deprecated_baz: bool,

        #[versioned(nested)]
        quux: Quux,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    pub(crate) struct Quux {
        corge: String,

        // An added field is tracked until it is removed.
        #[versioned(added(in = "v1beta1"), removed(in = "v1"))]
        grault: String,
    }
}
// ---
fn main() {}
