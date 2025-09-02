use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
pub mod versioned {
    enum Foo {
        A { aa: usize },
        B { bb: bool },
    }

    enum Bar {
        A(A),
        B {},
    }

    struct A {}
}
// ---
fn main() {}
