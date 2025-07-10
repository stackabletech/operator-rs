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
    #[versioned(crd(group = "stackable.tech"))]
    pub struct Foo {
        #[versioned(
            added(since = "v1beta1"),
            changed(since = "v1", from_name = "bah", from_type = "u16")
        )]
        bar: usize,
        baz: bool,
    }
}

fn main() {}
