use crate::person::{Person, PersonVersion};

mod person;

#[test]
fn stored_apiversion() {
    let stored_apiversion = PersonVersion::V2;

    let merged_crd = Person::merged_crd(stored_apiversion).expect("the CRDs must be mergeable");

    // We ensure that the merged CRD contains at least one version marked as
    // storage = true.
    let crd = merged_crd
        .spec
        .versions
        .iter()
        .find(|crd| crd.storage)
        .expect("The merged CRD must contain at least one version marked with storage = true");

    // This asserts that the name (version) of the CRD matches the one we expect
    assert_eq!(crd.name, stored_apiversion.as_version_str());
}
