//! Utility functions for processing data in the YAML file format
use serde::ser;

use crate::error::OperatorResult;

/// Serializes the given data structure as an explicit YAML document.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to return an error.
pub fn to_explicit_document_string<T>(value: &T) -> OperatorResult<String>
where
    T: ?Sized + ser::Serialize,
{
    let bare_document = serde_yaml::to_string(value)?;
    let explicit_document = format!("---\n{}", bare_document);

    Ok(explicit_document)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn value_can_be_serialized_to_an_explicit_document_string() {
        let value: BTreeMap<_, _> = [("key", "value")].into();

        let actual_yaml = to_explicit_document_string(&value).expect("serializable value");

        let expected_yaml = "\
            ---\n\
            key: value\n";

        assert_eq!(expected_yaml, actual_yaml);
    }
}
