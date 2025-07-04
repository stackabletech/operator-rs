use insta::glob;
use kube::core::response::StatusSummary;

use crate::person::{Person, PersonVersion};

mod person;

#[test]
fn person_v3_v1alpha1_v3() {
    glob!("./inputs/roundtrip", "*.json", |path| {
        // Convert from v3 to v1alpha1
        // NOTE (@Techassi): It should be noted that the input conversion review
        // contains a status with empty changedValues to be able to assert_eq
        // the objects at the end. As mentioned in the actual macro code, we
        // should avoid "polluting" the status if it is empty.
        let (request_v1alpha1, response_v1alpha1) = person::convert_via_file(path);
        let response = response_v1alpha1
            .response
            .as_ref()
            .expect("v1alpha1 review must have a response");

        assert_eq!(response.result.status, Some(StatusSummary::Success));

        // Construct the roundtrip review
        let roundtrip_review =
            person::roundtrip_conversion_review(response_v1alpha1, PersonVersion::V3);

        // Convert back to v3 from v1alpha1
        let response_v3 = Person::try_convert(roundtrip_review);
        let response = response_v3
            .response
            .as_ref()
            .expect("v3 review must have a response");

        assert_eq!(response.result.status, Some(StatusSummary::Success));

        // Now let compare the object how it started out with the object which
        // was produced through the conversion roundtrip. They must match.
        let original_object = request_v1alpha1
            .request
            .as_ref()
            .expect("v1alpha1 review must have a request")
            .objects
            .first()
            .expect("there must be at least one object");

        let converted_object = response
            .converted_objects
            .first()
            .expect("there must be at least one object");

        assert_eq!(original_object, converted_object);
    });
}
