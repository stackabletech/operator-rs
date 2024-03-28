//! Utility functions for processing data in the YAML file format
use std::io::Write;

use serde::ser;
use snafu::{ResultExt, Snafu};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to serialize YAML"))]
    SerializeYaml { source: serde_yaml::Error },

    #[snafu(display("failed to write YAML document separator"))]
    WriteDocumentSeparator { source: std::io::Error },
}

/// Serializes the given data structure as an explicit YAML document and writes it to a [`Write`].
///
/// Enums are serialized as a YAML map containing one entry in which the key identifies the variant
/// name.
///
/// # Example
///
/// ```
/// use serde::Serialize;
/// use stackable_operator::yaml;
///
/// #[derive(Serialize)]
/// #[serde(rename_all = "camelCase")]
/// enum Connection {
///     Inline(String),
///     Reference(String),
/// }
///
/// #[derive(Serialize)]
/// struct Spec {
///     connection: Connection,
/// }
///
/// let value = Spec {
///     connection: Connection::Inline("http://localhost".into()),
/// };
///
/// let mut buf = Vec::new();
/// yaml::serialize_to_explicit_document(&mut buf, &value).unwrap();
/// let actual_yaml = std::str::from_utf8(&buf).unwrap();
///
/// let expected_yaml = "---
/// connection:
///   inline: http://localhost
/// ";
///
/// assert_eq!(expected_yaml, actual_yaml);
/// ```
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to return an error.
pub fn serialize_to_explicit_document<T, W>(mut writer: W, value: &T) -> Result<()>
where
    T: ser::Serialize,
    W: Write,
{
    writer
        .write_all(b"---\n")
        .context(WriteDocumentSeparatorSnafu)?;
    let mut serializer = serde_yaml::Serializer::new(writer);
    serde_yaml::with::singleton_map_recursive::serialize(value, &mut serializer)
        .context(SerializeYamlSnafu)?;
    Ok(())
}
