#[versioned(version(name = "v1alpha1"), version(name = "v1"))]
// ---
mod versioned {
    mod v1alpha1 {
        pub use my::reexport::v1alpha1::*;
    }

    struct Foo {
        bar: usize,
    }
}
