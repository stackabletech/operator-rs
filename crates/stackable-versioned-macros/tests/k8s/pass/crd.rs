use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use stackable_versioned_macros::versioned;

#[allow(deprecated)]
fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1beta1"),
        version(name = "v1"),
        k8s(group = "stackable.tech")
    )]
    #[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
    pub struct FooSpec {
        #[versioned(
            added(since = "v1beta1"),
            changed(since = "v1", from_name = "bah", from_type = "u16")
        )]
        bar: usize,
        baz: bool,
    }

    let merged_crd = Foo::merged_crd(Version::V1).unwrap();
    println!("{}", serde_yaml::to_string(&merged_crd).unwrap());
}
