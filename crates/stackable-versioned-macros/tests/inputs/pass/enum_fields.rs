use stackable_versioned::versioned;
// ---
#[versioned(version(name = "v1alpha1"), version(name = "v1alpha2"))]
// ---
pub mod versioned {
    enum Foo {
        #[versioned(changed(in = "v1alpha2", previous_name = "PrevA"))]
        A {
            aa: usize,
            aaa: u64,
        },
        B {
            bb: bool,
        },
    }

    enum Bar {
        #[versioned(changed(in = "v1alpha2", previous_name = "PrevA"))]
        A(A),
        B {},
    }

    struct A {}
}
// ---
fn main() {}
