use serde_json::json;

use crate::person::{Person, PersonStatusWithChangedValues, PersonVersion};

mod person;

// Objects created directly in the storage version never pass through the conversion webhook, so
// their status doesn't contain any changed values. Operators must still be able to patch and read
// the status of such objects.
#[test]
fn status_does_not_require_changed_values() {
    let merged_crd = Person::merged_crd(PersonVersion::V2).expect("the CRDs must be mergeable");

    for version in merged_crd.spec.versions {
        let status_schema = version
            .schema
            .and_then(|schema| schema.open_api_v3_schema)
            .and_then(|schema| schema.properties)
            .and_then(|mut properties| properties.remove("status"))
            .expect("the CRD must contain a status schema");

        assert!(
            !status_schema
                .required
                .unwrap_or_default()
                .contains(&"changedValues".to_owned()),
            "changedValues must not be required in the status of version {version}",
            version = version.name
        );
    }

    let status: PersonStatusWithChangedValues = serde_json::from_value(json!({ "alive": false }))
        .expect("the status must deserialize without changedValues");
    assert!(status.changed_values.is_empty());

    // Empty changed values are not serialized
    assert_eq!(
        serde_json::to_value(&status).expect("the status must serialize"),
        json!({ "alive": false })
    );
}
