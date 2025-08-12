use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1"))]
// ---
mod versioned {
    mod v1alpha1 {
        #[allow(unused_imports)]
        pub use my::reexport::v1alpha1::*;
    }

    struct Foo {
        bar: usize,
    }
}
// ---
fn main() {}

mod my {
    pub mod reexport {
        pub mod v1alpha1 {}
    }
}
