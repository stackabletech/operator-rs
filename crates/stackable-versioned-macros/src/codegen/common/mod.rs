use std::collections::BTreeMap;

use k8s_version::Version;
use proc_macro2::Span;
use quote::format_ident;
use syn::Ident;

use crate::{
    attrs::common::{ModuleAttributes, StandaloneContainerAttributes},
    consts::{DEPRECATED_FIELD_PREFIX, DEPRECATED_VARIANT_PREFIX},
};

mod container;
mod item;
mod module;

pub(crate) use container::*;
pub(crate) use item::*;
pub(crate) use module::*;

/// Type alias to make the type of the version chain easier to handle.
pub(crate) type VersionChain = BTreeMap<Version, ItemStatus>;

#[derive(Debug, Clone)]
pub(crate) struct VersionDefinition {
    /// Indicates that the container version is deprecated.
    pub(crate) deprecated: bool,

    /// Indicates that the generation of `From<OLD> for NEW` should be skipped.
    pub(crate) skip_from: bool,

    /// A validated Kubernetes API version.
    pub(crate) inner: Version,

    /// The ident of the container.
    pub(crate) ident: Ident,

    /// Store additional doc-comment lines for this version.
    pub(crate) version_specific_docs: Vec<String>,
}

/// Converts lines of doc-comments into a trimmed list.
fn process_docs(input: &Option<String>) -> Vec<String> {
    if let Some(input) = input {
        input
            // Trim the leading and trailing whitespace, deleting suprefluous
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

// NOTE (@Techassi): Can we maybe unify these two impls?
impl From<&StandaloneContainerAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &StandaloneContainerAttributes) -> Self {
        attributes
            .versions
            .iter()
            .map(|v| VersionDefinition {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: Ident::new(&v.name.to_string(), Span::call_site()),
                version_specific_docs: process_docs(&v.doc),
                deprecated: v.deprecated.is_present(),
                inner: v.name,
            })
            .collect()
    }
}

impl From<&ModuleAttributes> for Vec<VersionDefinition> {
    fn from(attributes: &ModuleAttributes) -> Self {
        attributes
            .versions
            .iter()
            .map(|v| VersionDefinition {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: format_ident!("{version}", version = v.name.to_string()),
                version_specific_docs: process_docs(&v.doc),
                deprecated: v.deprecated.is_present(),
                inner: v.name,
            })
            .collect()
    }
}

/// Removes the deprecated prefix from a field ident.
///
/// See [`DEPRECATED_FIELD_PREFIX`].
pub(crate) fn remove_deprecated_field_prefix(ident: &Ident) -> Ident {
    let ident = ident.to_string();
    let ident = ident.trim_start_matches(DEPRECATED_FIELD_PREFIX);

    format_ident!("{ident}")
}

/// Removes the deprecated prefix from a variant ident.
///
/// See [`DEPRECATED_VARIANT_PREFIX`].
pub(crate) fn remove_deprecated_variant_prefix(ident: &Ident) -> Ident {
    // NOTE (@Techassi): Currently Clippy only issues a warning for variants
    // with underscores in their name. That's why we additionally remove the
    // leading underscore from the ident to use the expected name during code
    // generation.
    let ident = ident.to_string();
    let ident = ident
        .trim_start_matches(DEPRECATED_VARIANT_PREFIX)
        .trim_start_matches('_');

    format_ident!("{ident}")
}
