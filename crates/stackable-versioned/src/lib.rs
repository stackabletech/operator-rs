//! This crate enables versioning of structs and enums through procedural macros.
//!
//! Currently supported versioning schemes:
//!
//! - Kubernetes API versions (eg: `v1alpha1`, `v1beta1`, `v1`, `v2`), with optional support for
//!   generating CRDs.
//!
//! Support will be extended to SemVer versions, as well as custom version formats in the future.
//!
//! See [`versioned`] for an in-depth usage guide and a list of supported arguments.

use std::collections::HashMap;

use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use snafu::{ErrorCompat, Snafu};
// Re-export
pub use stackable_versioned_macros::versioned;

/// A value-to-value conversion that consumes the input value while tracking changes via a
/// Kubernetes status.
///
/// This allows nested sub structs to bubble up their tracked changes.
pub trait TrackingFrom<T, S>
where
    Self: Sized,
    S: TrackingStatus + Default,
{
    /// Convert `T` into `Self`.
    fn tracking_from(value: T, status: &mut S, parent: &str) -> Self;
}

/// A value-to-value conversion that consumes the input value while tracking changes via a
/// Kubernetes status. The opposite of [`TrackingFrom`].
///
/// One should avoid implementing [`TrackingInto`] as it is automatically implemented via a
/// blanket implementation.
pub trait TrackingInto<T, S>
where
    Self: Sized,
    S: TrackingStatus + Default,
{
    /// Convert `Self` into `T`.
    fn tracking_into(self, status: &mut S, parent: &str) -> T;
}

impl<T, U, S> TrackingInto<U, S> for T
where
    S: TrackingStatus + Default,
    U: TrackingFrom<T, S>,
{
    fn tracking_into(self, status: &mut S, parent: &str) -> U {
        U::tracking_from(self, status, parent)
    }
}

/// Used to access [`ChangedValues`] from any status.
pub trait TrackingStatus {
    fn changes(&mut self) -> &mut ChangedValues;
}

// NOTE (@Techassi): This struct represents a rough first draft of how tracking values across
// CRD versions can be achieved. It might change down the line.
// FIXME (@Techassi): Ideally we don't serialize empty maps. Further, we shouldn't serialize the
// changedValues field in the status, if there it is empty. This currently "pollutes" the status
// with empty JSON objects.
/// Contains changed values during upgrades and downgrades of CRDs.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct ChangedValues {
    /// List of values needed when downgrading to a particular version.
    pub downgrades: HashMap<String, Vec<ChangedValue>>,

    /// List of values needed when upgrading to a particular version.
    pub upgrades: HashMap<String, Vec<ChangedValue>>,
    // TODO (@Techassi): Add a version indicator here if we ever decide to change the tracking
    // mechanism.
}

/// Contains a changed value for a single field of the CRD.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangedValue {
    /// The name of the field of the custom resource this value is for.
    pub field_name: String,

    /// The value to be used when upgrading or downgrading the custom resource.
    #[schemars(schema_with = "raw_object_schema")]
    pub value: serde_yaml::Value,
}

// TODO (@Techassi): Think about where this should live. Basically this already exists in
// stackable-operator, but we cannot use it without depending on it which I would like to
// avoid.
fn raw_object_schema(_: &mut schemars::r#gen::SchemaGenerator) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
        extensions: [(
            "x-kubernetes-preserve-unknown-fields".to_owned(),
            serde_json::Value::Bool(true),
        )]
        .into(),
        ..Default::default()
    })
}

/// This error indicates that parsing an object from a conversion review failed.
#[derive(Debug, Snafu)]
pub enum ParseObjectError {
    #[snafu(display("the field {field:?} is missing"))]
    FieldNotPresent { field: String },

    #[snafu(display("the field {field:?} must be a string"))]
    FieldNotStr { field: String },

    #[snafu(display("encountered unknown object API version {api_version:?}"))]
    UnknownApiVersion { api_version: String },

    #[snafu(display("failed to deserialize object from JSON"))]
    Deserialize { source: serde_json::Error },

    #[snafu(display("unexpected object kind {kind:?}, expected {expected:?}"))]
    UnexpectedKind { kind: String, expected: String },
}

/// This error indicates that converting an object from a conversion review to the desired
/// version failed.
#[derive(Debug, Snafu)]
pub enum ConvertObjectError {
    #[snafu(display("failed to parse object"))]
    Parse { source: ParseObjectError },

    #[snafu(display("failed to serialize object into json"))]
    Serialize { source: serde_json::Error },

    #[snafu(display("failed to parse desired API version"))]
    ParseDesiredApiVersion {
        source: UnknownDesiredApiVersionError,
    },
}

impl ConvertObjectError {
    /// Joins the error and its sources using colons.
    pub fn join_errors(&self) -> String {
        // NOTE (@Techassi): This can be done with itertools in a way shorter
        // fashion but obviously brings in another dependency. Which of those
        // two solutions performs better needs to evaluated.
        // self.iter_chain().join(": ")
        self.iter_chain()
            .map(|err| err.to_string())
            .collect::<Vec<String>>()
            .join(": ")
    }

    /// Returns a HTTP status code based on the underlying error.
    pub fn http_status_code(&self) -> u16 {
        match self {
            ConvertObjectError::Parse { .. } => 400,
            ConvertObjectError::Serialize { .. } => 500,

            // This is likely the clients fault, as it is requesting a unsupported version
            ConvertObjectError::ParseDesiredApiVersion {
                source: UnknownDesiredApiVersionError { .. },
            } => 400,
        }
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("unknown API version {api_version:?}"))]
pub struct UnknownDesiredApiVersionError {
    pub api_version: String,
}

pub fn jthong_path(parent: &str, child: &str) -> String {
    format!("{parent}.{child}")
}
