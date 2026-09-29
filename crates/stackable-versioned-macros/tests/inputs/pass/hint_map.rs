use std::collections::HashMap;

use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
pub(crate) mod versioned {
    pub(crate) struct Foo {
        #[versioned(
            changed(since = "v1alpha2", from_type = "HashMap<String, u8>"),
            hint(map)
        )]
        bar: HashMap<String, Bar>,
    }
}
// ---
struct Bar(u8);

impl From<u8> for Bar {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<Bar> for u8 {
    fn from(value: Bar) -> Self {
        value.0
    }
}

fn main() {}
