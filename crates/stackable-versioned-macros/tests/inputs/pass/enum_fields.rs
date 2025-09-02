use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
pub mod versioned {
    enum Foo {
        A { aa: usize, aaa: u64 },
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
