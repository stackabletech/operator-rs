//! Utility functions for processing data in the YAML file format
use std::{io::Write, path::Path, str::FromStr};

use semver::Version;
use snafu::{ResultExt, Snafu};

const STACKABLE_DOCS_HOME_URL_PLACEHOLDER: &str = "DOCS_BASE_URL_PLACEHOLDER";
const STACKABLE_DOCS_HOME_BASE_URL: &str = "https://docs.stackable.tech/home";

type Result<T, E = Error> = std::result::Result<T, E>;

/// Represents every error which can be encountered during YAML serialization.
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to serialize YAML"))]
    SerializeYaml { source: serde_yaml::Error },

    #[snafu(display("failed to write YAML document separator"))]
    WriteDocumentSeparator { source: std::io::Error },

    #[snafu(display("failed to write YAML to file"))]
    WriteToFile { source: std::io::Error },

    #[snafu(display("failed to write YAML to stdout"))]
    WriteToStdout { source: std::io::Error },

    #[snafu(display("failed to parse {input:?} as semantic version"))]
    ParseSemanticVersion {
        source: semver::Error,
        input: String,
    },

    #[snafu(display("failed to parse bytes as valid UTF-8 string"))]
    ParseUtf8Bytes { source: std::string::FromUtf8Error },
}

pub(crate) struct DocUrlReplacer<'a>(&'a str);

impl<'a> DocUrlReplacer<'a> {
    pub(crate) fn new(operator_version: &'a str) -> Self {
        Self(operator_version)
    }

    fn replace(&self, input: &str) -> Result<String> {
        let docs_version = match self.0 {
            "0.0.0-dev" => "nightly".to_owned(),
            ver => {
                let v = Version::from_str(ver).context(ParseSemanticVersionSnafu { input })?;
                format!("{major}.{minor}", major = v.major, minor = v.minor)
            }
        };

        Ok(input.replace(
            STACKABLE_DOCS_HOME_URL_PLACEHOLDER,
            &format!("{STACKABLE_DOCS_HOME_BASE_URL}/{docs_version}"),
        ))
    }
}

/// Provides configurable options during YAML serialization.
///
/// For most people the default implementation [`SerializeOptions::default()`] is sufficient as it
/// enables explicit document and singleton map serialization.
pub struct SerializeOptions {
    /// Adds leading triple dashes (`---`) to the output string.
    pub explicit_document: bool,

    /// Serialize enum variants as YAML maps using the variant name as the key.
    pub singleton_map: bool,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            explicit_document: true,
            singleton_map: true,
        }
    }
}

/// Serializes any type `T` which is [serializable](serde::Serialize) as YAML using the provided
/// [`SerializeOptions`].
///
/// It additionally replaces the documentation URL placeholder with the correct value based on the
/// provided `operator_version`.
pub trait YamlSchema: Sized + serde::Serialize {
    /// Generates the YAML schema of `self` using the provided [`SerializeOptions`].
    fn generate_yaml_schema(
        &self,
        operator_version: &str,
        options: SerializeOptions,
    ) -> Result<String> {
        let mut buffer = Vec::new();

        serialize(&self, &mut buffer, options)?;

        let yaml_string = String::from_utf8(buffer).context(ParseUtf8BytesSnafu)?;

        let replacer = DocUrlReplacer::new(operator_version);
        let yaml_string = replacer.replace(&yaml_string)?;

        Ok(yaml_string)
    }

    /// Generates and write the YAML schema of `self` to a file at `path` using the provided
    /// [`SerializeOptions`].
    fn write_yaml_schema<P: AsRef<Path>>(
        &self,
        path: P,
        operator_version: &str,
        options: SerializeOptions,
    ) -> Result<()> {
        let schema = self.generate_yaml_schema(operator_version, options)?;
        std::fs::write(path, schema).context(WriteToFileSnafu)
    }

    /// Generates and prints the YAML schema of `self` to stdout at `path` using the provided
    /// [`SerializeOptions`].
    fn print_yaml_schema(&self, operator_version: &str, options: SerializeOptions) -> Result<()> {
        let schema = self.generate_yaml_schema(operator_version, options)?;

        let mut writer = std::io::stdout();
        writer
            .write_all(schema.as_bytes())
            .context(WriteToStdoutSnafu)
    }
}

impl<T> YamlSchema for T where T: serde::ser::Serialize {}

/// Provides YAML schema generation and output capabilities for Kubernetes custom resources.
pub trait CustomResourceExt: kube::CustomResourceExt {
    /// Generates the YAML schema of a `CustomResourceDefinition` and writes it to the specified
    /// file at `path`.
    ///
    /// It additionally replaces the documentation URL placeholder with the correct value based on
    /// the provided `operator_version`. The written YAML string is an explicit document with
    /// leading dashes (`---`).
    fn write_yaml_schema<P: AsRef<Path>>(path: P, operator_version: &str) -> Result<()> {
        Self::crd().write_yaml_schema(path, operator_version, SerializeOptions::default())
    }

    /// Generates the YAML schema of a `CustomResourceDefinition` and prints it to [stdout].
    ///
    /// It additionally replaces the documentation URL placeholder with the correct value based on
    /// the provided `operator_version`. The written YAML string is an explicit document with
    /// leading dashes (`---`).
    ///
    /// [stdout]: std::io::stdout
    fn print_yaml_schema(operator_version: &str) -> Result<()> {
        Self::crd().print_yaml_schema(operator_version, SerializeOptions::default())
    }

    /// Generates the YAML schema of a `CustomResourceDefinition` and returns it as a [`String`].
    fn yaml_schema(operator_version: &str) -> Result<String> {
        Self::crd().generate_yaml_schema(operator_version, SerializeOptions::default())
    }
}

impl<T> CustomResourceExt for T where T: kube::CustomResourceExt {}

/// Serializes the given data structure and writes it to a [`Writer`](Write).
pub fn serialize<T, W>(value: &T, mut writer: W, options: SerializeOptions) -> Result<()>
where
    T: serde::Serialize,
    W: std::io::Write,
{
    if options.explicit_document {
        writer
            .write_all(b"---\n")
            .context(WriteDocumentSeparatorSnafu)?;
    }

    let mut serializer = serde_yaml::Serializer::new(writer);

    if options.singleton_map {
        serde_yaml::with::singleton_map_recursive::serialize(value, &mut serializer)
            .context(SerializeYamlSnafu)?;
    } else {
        value
            .serialize(&mut serializer)
            .context(SerializeYamlSnafu)?;
    }

    Ok(())
}
