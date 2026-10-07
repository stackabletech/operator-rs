use kube::api::ObjectMeta;

/// Annotation an agent sets on an object once it created or adopted the resource in the product.
///
/// The value is the RFC 3339 timestamp of that moment. Agents only delete resources they own.
pub const OWNED_SINCE_ANNOTATION: &str = "stackable.tech/owned-since";

/// Whether the agent owns the resource in the product that the object with this metadata manages.
pub fn is_owned(metadata: &ObjectMeta) -> bool {
    metadata
        .annotations
        .as_ref()
        .is_some_and(|annotations| annotations.contains_key(OWNED_SINCE_ANNOTATION))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::no_annotations(None, false)]
    #[case::other_annotation(Some(("stackable.tech/other", "x")), false)]
    #[case::owned(Some((OWNED_SINCE_ANNOTATION, "2026-10-06T09:12:44Z")), true)]
    fn ownership(#[case] annotation: Option<(&str, &str)>, #[case] expected: bool) {
        let metadata = ObjectMeta {
            annotations: annotation
                .map(|(key, value)| BTreeMap::from([(key.to_owned(), value.to_owned())])),
            ..ObjectMeta::default()
        };

        assert_eq!(expected, is_owned(&metadata));
    }
}
