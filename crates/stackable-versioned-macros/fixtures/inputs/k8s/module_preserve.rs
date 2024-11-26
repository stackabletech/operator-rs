#[versioned(
    version(name = "v1alpha1"),
    version(name = "v1"),
    version(name = "v2alpha1"),
    preserve_module
)]
// ---
pub(crate) mod versioned {
    #[versioned(k8s(group = "foo.example.org", plural = "foos", namespaced))]
    pub struct FooSpec {
        bar: usize,

        #[versioned(added(since = "v1"))]
        baz: bool,

        #[versioned(deprecated(since = "v2alpha1"))]
        deprecated_foo: String,
    }

    #[versioned(k8s(group = "bar.example.org", plural = "bars"))]
    pub struct BarSpec {
        baz: String,
    }

    pub struct Baz {
        boom: Option<u16>,
    }
}
