use stackable_versioned_macros::versioned;

fn main() {
    #[versioned(
        version(name = "v1alpha1"),
        version(name = "v1alpha2"),
        version(name = "v1beta1"),
        version(name = "v1")
    )]
    struct Foo {
        username: String,

        #[versioned(added(since = "v1alpha2", default = default_foo))]
        first_name: String,

        #[versioned(added(since = "v1beta1"))]
        last_name: String,
    }
}

fn default_foo() -> String {
    "foo".into()
}
