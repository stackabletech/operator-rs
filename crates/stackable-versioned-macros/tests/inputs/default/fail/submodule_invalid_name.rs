use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(version(name = "v1alpha1"))]
    mod versioned {
        mod v1alpha2 {}
    }
}
