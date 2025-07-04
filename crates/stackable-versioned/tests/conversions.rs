use insta::{assert_snapshot, glob};
use kube::core::response::StatusSummary;

mod person;

#[test]
fn pass() {
    glob!("./inputs/conversions/pass/", "*.json", |path| {
        let (request, response) = person::convert_via_file(path);

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
        let (request, response) = person::convert_via_file(path);

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
