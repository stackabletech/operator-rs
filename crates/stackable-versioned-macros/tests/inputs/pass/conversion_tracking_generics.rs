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
    #[versioned(crd(
        group = "stackable.tech",
        status = FooStatus,
        doc = "Test",
    ))]
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, CustomResource)]
    pub(crate) struct FooSpec {
        #[versioned(added(since = "v1beta1"), changed(since = "v1", from_name = "bah"))]
        bar: usize,

        #[versioned(nested)]
        baz: Baz<String>,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    #[allow(dead_code)]
    pub(crate) struct Baz<B>
    where
        B: Clone + Default,
    {
        inner_1: bool,

        // FIXME (@Techassi): This currently leads to errors, because the generic type B is emitted
        // for v1alpha1 as well. There it is unused. We need to somehow figure out if a generic type
        // is used and if so, emit it. Otherwise, don't emit it.
        // #[versioned(added(since = "v1beta1"))]
        inner_2: B,
    }
}
// ---
fn main() {}

#[derive(Clone, Debug, Default, JsonSchema, Deserialize, Serialize)]
pub struct FooStatus {
    foo: String,
}

fn usize_to_u16(input: usize) -> u16 {
    input.try_into().unwrap()
}
