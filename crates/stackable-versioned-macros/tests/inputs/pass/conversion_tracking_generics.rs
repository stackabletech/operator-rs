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
    // The generic type is named S on purpose, to ensure it doesn't collide with the generic type
    // used for the tracking status in the generated TrackingFrom impls.
    pub(crate) struct Baz<S>
    where
        S: Clone + Default,
    {
        inner_1: bool,

        // FIXME (@Techassi): This currently leads to errors, because the generic type S is emitted
        // for v1alpha1 as well. There it is unused. We need to somehow figure out if a generic type
        // is used and if so, emit it. Otherwise, don't emit it.
        // #[versioned(added(since = "v1beta1"))]
        inner_2: S,

        // Concrete type on purpose, see the FIXME above. Added in v1 (not v1beta1) so it doesn't
        // share the v1beta1 upgrade bucket with FooSpec::bar.
        #[versioned(added(since = "v1"))]
        inner_3: String,
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
