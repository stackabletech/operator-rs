use stackable_versioned::versioned;

fn main() {
    #[versioned(version(name = "v1alpha1"))]
    mod versioned {
        mod v1alpha1 {
            struct Foo;
        }
    }
}
