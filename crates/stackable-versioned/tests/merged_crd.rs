use crate::person::{Person, PersonVersion};

mod person;

#[test]
fn stored_apiversion() {
    let desired_stored_apiversion = PersonVersion::V2;

    let merged_crd =
        Person::merged_crd(desired_stored_apiversion).expect("the CRDs must be mergeable");

    // First, we ensure that all storage fields have the correct value.
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

    // Lastly, we ensure the first version (which is always the stored version)
    // is the one we expect.
    let stored_apiversion = &merged_crd
        .spec
        .versions
        .first()
        .expect("there must be at least one CRD version")
        .name;

    assert_eq!(
        stored_apiversion,
        desired_stored_apiversion.as_version_str()
    );
}
