//! Utility functions for processing data in the YAML file format
use serde::ser;

use crate::error::{Error, OperatorResult};

/// A YAML document type
///
/// For a detailled description, see the
/// [YAML specification](https://yaml.org/spec/1.2.2/#rule-l-any-document).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentType {
    DirectiveDocument,
    ExplicitDocument,
    BareDocument,
}

/// Serializes the given data structure as an explicit YAML document.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to return an error.
///
/// An [`Error::UnsupportedYamlDocumentError`] is returned if the used `serde_yaml` version
/// generates a directive document.
pub fn to_explicit_document_string<T>(value: &T) -> OperatorResult<String>
where
    T: ?Sized + ser::Serialize,
{
    // The returned document type depends on the serde_yaml version.
    let document = serde_yaml::to_string(value)?;

    match determine_document_type(&document) {
        DocumentType::DirectiveDocument => Err(Error::UnsupportedYamlDocumentError {
            message: "serde_yaml::to_string generated a directive document which cannot be \
                converted to an explicit document."
                .into(),
        }),
        DocumentType::ExplicitDocument => Ok(document),
        DocumentType::BareDocument => Ok(format!("---\n{}", document)),
    }
}

/// Determines the type of the given YAML document.
///
/// It is assumend that the given string contains a valid YAML document.
pub fn determine_document_type(document: &str) -> DocumentType {
    if document.starts_with('%') {
        DocumentType::DirectiveDocument
    } else if document.starts_with("---") {
        DocumentType::ExplicitDocument
    } else {
        DocumentType::BareDocument
    }
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

    #[test]
    fn document_type_can_be_determined() {
        let directive_document = "\
            %YAML 1.2\n\
            ---\n\
            key: value";
        let explicit_document = "\
            ---\n\
            key: value";
        let bare_document = "\
            key: value";

        let directive_document_type = determine_document_type(directive_document);
        let explicit_document_type = determine_document_type(explicit_document);
        let bare_document_type = determine_document_type(bare_document);

        assert_eq!(DocumentType::DirectiveDocument, directive_document_type);
        assert_eq!(DocumentType::ExplicitDocument, explicit_document_type);
        assert_eq!(DocumentType::BareDocument, bare_document_type);
    }
}
