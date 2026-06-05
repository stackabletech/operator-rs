//! Writers for Hadoop XML config files and Java `.properties` files.
//!
//! Originally part of the `product-config` crate's `writer` module; previously
//! vendored into the individual operators, now provided here as the shared home so
//! operators do not depend on `product-config` for rendering.

use std::{fmt::Write as _, io::Write};

use java_properties::{PropertiesError, PropertiesWriter};
use snafu::{ResultExt, Snafu};
use xml::escape::escape_str_attribute;

#[derive(Debug, Snafu)]
pub enum PropertiesWriterError {
    #[snafu(display("failed to create properties file"))]
    Properties { source: PropertiesError },

    #[snafu(display("failed to convert properties file byte array to UTF-8"))]
    FromUtf8 { source: std::string::FromUtf8Error },
}

/// Creates a common Java properties file string in the format:
/// `property_1=value_1\nproperty_2=value_2\n`.
pub fn to_java_properties_string<'a, T>(properties: T) -> Result<String, PropertiesWriterError>
where
    T: Iterator<Item = (&'a String, &'a Option<String>)>,
{
    let mut output = Vec::new();
    write_java_properties(&mut output, properties)?;
    String::from_utf8(output).context(FromUtf8Snafu)
}

/// Writes Java properties to the given writer. A `None` value is written as an
/// empty value (`key=`).
fn write_java_properties<'a, W, T>(writer: W, properties: T) -> Result<(), PropertiesWriterError>
where
    W: Write,
    T: Iterator<Item = (&'a String, &'a Option<String>)>,
{
    let mut writer = PropertiesWriter::new(writer);
    for (k, v) in properties {
        let property_value = v.as_deref().unwrap_or_default();
        writer.write(k, property_value).context(PropertiesSnafu)?;
    }
    writer.flush().context(PropertiesSnafu)?;
    Ok(())
}

/// Converts properties into a Hadoop configuration XML, including the wrapping
/// `<configuration>...</configuration>` elements. Properties with a `None` value
/// are skipped. Keys and values are XML-escaped.
pub fn to_hadoop_xml<'a, T>(properties: T) -> String
where
    T: Iterator<Item = (&'a String, &'a Option<String>)>,
{
    let mut snippet = String::new();
    for (k, v) in properties {
        let escaped_value = match v {
            Some(value) => escape_str_attribute(value),
            None => continue,
        };
        let escaped_key = escape_str_attribute(k);
        write!(
            snippet,
            "  <property>\n    <name>{escaped_key}</name>\n    <value>{escaped_value}</value>\n  </property>\n"
        )
        .expect("writing to a String is infallible");
    }
    format!("<?xml version=\"1.0\"?>\n<configuration>\n{snippet}</configuration>")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn xml(pairs: &[(&str, Option<&str>)]) -> String {
        let map: BTreeMap<String, Option<String>> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
            .collect();
        to_hadoop_xml(map.iter())
    }

    fn props(pairs: &[(&str, Option<&str>)]) -> String {
        let map: BTreeMap<String, Option<String>> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
            .collect();
        to_java_properties_string(map.iter()).unwrap()
    }

    #[test]
    fn hadoop_xml_wraps_empty_configuration() {
        assert_eq!(
            xml(&[]),
            "<?xml version=\"1.0\"?>\n<configuration>\n</configuration>"
        );
    }

    #[test]
    fn hadoop_xml_renders_single_property() {
        assert_eq!(
            xml(&[("fs.defaultFS", Some("hdfs://hdfs/"))]),
            "<?xml version=\"1.0\"?>\n<configuration>\n  \
             <property>\n    <name>fs.defaultFS</name>\n    \
             <value>hdfs://hdfs/</value>\n  </property>\n</configuration>"
        );
    }

    #[test]
    fn hadoop_xml_skips_none_values() {
        assert_eq!(
            xml(&[("kept", Some("1")), ("dropped", None)]),
            "<?xml version=\"1.0\"?>\n<configuration>\n  \
             <property>\n    <name>kept</name>\n    \
             <value>1</value>\n  </property>\n</configuration>"
        );
    }

    #[test]
    fn hadoop_xml_escapes_special_characters() {
        let rendered = xml(&[("k", Some("<a>&b"))]);
        assert!(
            rendered.contains("<value>&lt;a&gt;&amp;b</value>"),
            "{rendered}"
        );
    }

    #[test]
    fn java_properties_renders_key_value() {
        assert_eq!(props(&[("a", Some("1")), ("b", Some("2"))]), "a=1\nb=2\n");
    }

    #[test]
    fn java_properties_renders_none_as_empty() {
        assert_eq!(props(&[("none", None)]), "none=\n");
    }

    #[test]
    fn java_properties_escapes_colon_in_value() {
        assert_eq!(
            props(&[("url", Some("file://this/location/file.abc"))]),
            "url=file\\://this/location/file.abc\n"
        );
    }
}
