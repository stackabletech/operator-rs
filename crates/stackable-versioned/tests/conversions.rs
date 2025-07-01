use std::{fs::File, path::Path};

use insta::{assert_snapshot, glob};
use kube::core::{conversion::ConversionReview, response::StatusSummary};

use crate::person::Person;

mod person;

#[test]
fn pass() {
    glob!("./inputs/conversions/pass/", "*.json", |path| {
        let (request, response) = convert_via_file(path);

        let formatted = serde_json::to_string_pretty(&response)
            .expect("Failed to serialize ConversionResponse");
        assert_snapshot!(formatted);

        let response = response
            .response
            .expect("ConversionReview had no response!");

        assert_eq!(
            response.result.status,
            Some(StatusSummary::Success),
            "File {path:?} should be converted successfully"
        );
        assert_eq!(request.request.unwrap().uid, response.uid);
    })
}

#[test]
fn fail() {
    glob!("./inputs/conversions/fail/", "*.json", |path| {
        let (request, response) = convert_via_file(path);

        let formatted = serde_json::to_string_pretty(&response)
            .expect("Failed to serialize ConversionResponse");
        assert_snapshot!(formatted);

        let response = response
            .response
            .expect("ConversionReview had no response!");

        assert_eq!(
            response.result.status,
            Some(StatusSummary::Failure),
            "File {path:?} should *not* be converted successfully"
        );
        if let Some(request) = &request.request {
            assert_eq!(request.uid, response.uid);
        }
    })
}

fn convert_via_file(path: &Path) -> (ConversionReview, ConversionReview) {
    let request: ConversionReview =
        serde_json::from_reader(File::open(path).expect("failed to open test file"))
            .expect("failed to parse ConversionReview from test file");
    let response = Person::try_convert(request.clone());

    (request, response)
}
