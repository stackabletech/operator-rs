use std::path::Path;

use snafu::{ResultExt, Snafu};

use crate::yaml::{SerializeOptions, YamlSchema};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to write CRD YAML schema to file"))]
    WriteToFile { source: crate::yaml::Error },

    #[snafu(display("failed to write CRD YAML schema to stdout"))]
    WriteToStdout { source: crate::yaml::Error },

    #[snafu(display("failed to generate CRD YAML schema"))]
    GenerateSchema { source: crate::yaml::Error },
}

/// Provides YAML schema generation and output capabilities for Kubernetes custom resources.
pub trait CustomResourceExt: kube::CustomResourceExt {
    /// Generates the YAML schema of a `CustomResourceDefinition` and writes it to the specified
    /// file at `path`.
    ///
    /// It additionally replaces the documentation URL placeholder with the correct value based on
    /// the provided `operator_version`. The written YAML string is an explicit document with
    /// leading dashes (`---`).
    fn write_yaml_schema<P: AsRef<Path>>(path: P, operator_version: &str) -> Result<()> {
        Self::crd()
            .write_yaml_schema(path, operator_version, SerializeOptions::default())
            .context(WriteToFileSnafu)
    }

    /// Generates the YAML schema of a `CustomResourceDefinition` and prints it to [stdout].
    ///
    /// It additionally replaces the documentation URL placeholder with the correct value based on
    /// the provided `operator_version`. The written YAML string is an explicit document with
    /// leading dashes (`---`).
    ///
    /// [stdout]: std::io::stdout
    fn print_yaml_schema(operator_version: &str) -> Result<()> {
        Self::crd()
            .print_yaml_schema(operator_version, SerializeOptions::default())
            .context(WriteToStdoutSnafu)
    }

    /// Generates the YAML schema of a `CustomResourceDefinition` and returns it as a [`String`].
    fn yaml_schema(operator_version: &str) -> Result<String> {
        Self::crd()
            .generate_yaml_schema(operator_version, SerializeOptions::default())
            .context(GenerateSchemaSnafu)
    }
}

impl<T> CustomResourceExt for T where T: kube::CustomResourceExt {}
