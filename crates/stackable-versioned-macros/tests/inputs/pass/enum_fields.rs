use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
pub mod versioned {
    enum MyEnum {
        A { aa: usize },
        B { bb: bool },
    }

    enum Bla {
        A(A),
    }

    struct A {}
}
// ---
fn main() {}
