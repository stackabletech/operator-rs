use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stackable_versioned_macros::versioned;

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
mod versioned {
    #[versioned(crd(group = "stackable.tech", doc = "Test"))]
    pub struct Foo {
        #[versioned(
            added(in = "v1beta1"),
            changed(in = "v1", previous_name = "bah", previous_type = "u16")
        )]
        bar: usize,
        baz: bool,
    }
}

fn main() {}
