//! Writers for Hadoop XML config files and Java `.properties` files.

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
    T: Iterator<Item = (&'a String, &'a String)>,
{
    let mut output = Vec::new();
    write_java_properties(&mut output, properties)?;
    String::from_utf8(output).context(FromUtf8Snafu)
}

/// Writes Java properties to the given writer.
fn write_java_properties<'a, W, T>(writer: W, properties: T) -> Result<(), PropertiesWriterError>
where
    W: Write,
    T: Iterator<Item = (&'a String, &'a String)>,
{
    let mut writer = PropertiesWriter::new(writer);
    for (k, v) in properties {
        writer.write(k, v).context(PropertiesSnafu)?;
    }
    writer.flush().context(PropertiesSnafu)?;
    Ok(())
}

/// Converts properties into a Hadoop configuration XML, including the wrapping
/// `<configuration>...</configuration>` elements.
pub fn to_hadoop_xml<'a, T>(properties: T) -> String
where
    T: Iterator<Item = (&'a String, &'a String)>,
{
    let mut snippet = String::new();
    for (k, v) in properties {
        let escaped_key = escape_str_attribute(k);
        let escaped_value = escape_str_attribute(v);
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

    fn xml(pairs: &[(&str, &str)]) -> String {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        to_hadoop_xml(map.iter())
    }

    fn props(pairs: &[(&str, &str)]) -> String {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
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
            xml(&[("fs.defaultFS", "hdfs://hdfs/")]),
            "<?xml version=\"1.0\"?>\n<configuration>\n  \
             <property>\n    <name>fs.defaultFS</name>\n    \
             <value>hdfs://hdfs/</value>\n  </property>\n</configuration>"
        );
    }

    #[test]
    fn hadoop_xml_escapes_special_characters() {
        let rendered = xml(&[("k", "<a>&b")]);
        assert!(
            rendered.contains("<value>&lt;a&gt;&amp;b</value>"),
            "{rendered}"
        );
    }

    #[test]
    fn java_properties_renders_key_value() {
        assert_eq!(props(&[("a", "1"), ("b", "2")]), "a=1\nb=2\n");
    }

    #[test]
    fn java_properties_renders_empty() {
        assert_eq!(props(&[("empty", "")]), "empty=\n");
    }

    #[test]
    fn java_properties_escapes_colon_in_value() {
        assert_eq!(
            props(&[("url", "file://this/location/file.abc")]),
            "url=file\\://this/location/file.abc\n"
        );
    }
}
