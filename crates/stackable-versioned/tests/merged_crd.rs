use crate::person::{Person, PersonVersion};

mod person;

#[test]
fn stored_apiversion() {
    let merged_crd = Person::merged_crd(PersonVersion::V2).expect("the CRDs must be mergeable");

    let all_storage_fields_correct =
        merged_crd
            .spec
            .versions
            .iter()
            .enumerate()
            .all(
                |(idx, crd)| {
                    if idx == 0 { crd.storage } else { !crd.storage }
                },
            );

    assert!(all_storage_fields_correct);
}
