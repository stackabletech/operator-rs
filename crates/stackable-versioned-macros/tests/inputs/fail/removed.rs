use stackable_versioned_macros::versioned;

#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1beta1"),
    version(name = "v1")
)]
mod versioned {
    struct RemovedInFirstVersion {
        #[versioned(removed(in = "v1alpha1"))]
        foo: usize,
    }

    struct RemovedInUnknownVersion {
        #[versioned(removed(in = "v2"))]
        foo: usize,
    }

    struct RemovedAndDeprecatedInSameVersion {
        #[versioned(deprecated(in = "v1"), removed(in = "v1"))]
        deprecated_foo: usize,
    }

    struct RemovedBeforeChange {
        #[versioned(changed(in = "v1", previous_name = "bar"), removed(in = "v1beta1"))]
        foo: usize,
    }

    enum RemovedVariant {
        #[versioned(removed(in = "v1"))]
        Foo,
    }
}

fn main() {}
