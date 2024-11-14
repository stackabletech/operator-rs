use darling::util::IdentString;
use k8s_version::Version;
use quote::format_ident;
use syn::{Path, Type};

use crate::attrs::{container::StandaloneContainerAttributes, module::ModuleAttributes};

pub(crate) mod changes;
pub(crate) mod container;
pub(crate) mod item;
pub(crate) mod module;

#[derive(Debug)]
pub(crate) struct VersionDefinition {
    /// Indicates that the container version is deprecated.
    pub(crate) deprecated: Option<String>,

    /// Indicates that the generation of `From<OLD> for NEW` should be skipped.
    pub(crate) skip_from: bool,

    /// A validated Kubernetes API version.
    pub(crate) inner: Version,

    /// The ident of the container.
    pub(crate) ident: IdentString,

    /// Store additional doc-comment lines for this version.
    pub(crate) docs: Vec<String>,
}

// NOTE (@Techassi): Can we maybe unify these two impls?
impl From<&StandaloneContainerAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &StandaloneContainerAttributes) -> Self {
        attributes
            .common_root_arguments
            .versions
            .iter()
            .map(|v| VersionDefinition {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: format_ident!("{version}", version = v.name.to_string()).into(),
                deprecated: v.deprecated.as_ref().map(|r#override| {
                    r#override.clone().unwrap_or(format!(
                        "Version {version} is deprecated",
                        version = v.name.to_string()
                    ))
                }),
                docs: process_docs(&v.doc),
                inner: v.name,
            })
            .collect()
    }
}

impl From<&ModuleAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &ModuleAttributes) -> Self {
        attributes
            .common_root_arguments
            .versions
            .iter()
            .map(|v| VersionDefinition {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: format_ident!("{version}", version = v.name.to_string()).into(),
                deprecated: v.deprecated.as_ref().map(|r#override| {
                    r#override.clone().unwrap_or(format!(
                        "Version {version} is deprecated",
                        version = v.name.to_string()
                    ))
                }),
                docs: process_docs(&v.doc),
                inner: v.name,
            })
            .collect()
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum ItemStatus {
    Addition {
        ident: IdentString,
        default_fn: Path,
        // NOTE (@Techassi): We need to carry idents and type information in
        // nearly every status. Ideally, we would store this in separate maps.
        ty: Type,
    },
    Change {
        from_ident: IdentString,
        to_ident: IdentString,
        from_type: Type,
        to_type: Type,
    },
    Deprecation {
        previous_ident: IdentString,
        note: Option<String>,
        ident: IdentString,
    },
    NoChange {
        previously_deprecated: bool,
        ident: IdentString,
        ty: Type,
    },
    NotPresent,
}

impl ItemStatus {
    pub(crate) fn get_ident(&self) -> &IdentString {
        match &self {
            ItemStatus::Addition { ident, .. } => ident,
            ItemStatus::Change { to_ident, .. } => to_ident,
            ItemStatus::Deprecation { ident, .. } => ident,
            ItemStatus::NoChange { ident, .. } => ident,
            ItemStatus::NotPresent => unreachable!(),
        }
    }
}

pub(crate) struct Change {
    pub(crate) item_ident: IdentString,
    pub(crate) item_type: Type,
    pub(crate) ty: ChangeType,
}

pub(crate) enum ChangeType {
    Added {
        default_fn: Path,
    },
    Changed {
        from_ident: IdentString,
        from_type: Type,
    },
    Deprecated {
        from_ident: IdentString,
        note: Option<String>,
    },
}

/// Converts lines of doc-comments into a trimmed list.
fn process_docs(input: &Option<String>) -> Vec<String> {
    if let Some(input) = input {
        input
            // Trim the leading and trailing whitespace, deleting superfluous
            // empty lines.
            .trim()
            .lines()
            // Trim the leading and trailing whitespace on each line that can be
            // introduced when the developer indents multi-line comments.
            .map(|line| line.trim().to_owned())
            .collect()
    } else {
        Vec::new()
    }
}
