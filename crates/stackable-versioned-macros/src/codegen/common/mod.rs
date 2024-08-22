use std::collections::BTreeMap;

use k8s_version::Version;
use proc_macro2::Span;
use quote::format_ident;
use syn::Ident;

use crate::{
    attrs::common::ContainerAttributes,
    consts::{DEPRECATED_FIELD_PREFIX, DEPRECATED_VARIANT_PREFIX},
};

mod container;
mod item;

pub(crate) use container::*;
pub(crate) use item::*;

/// Type alias to make the type of the version chain easier to handle.
pub(crate) type VersionChain = BTreeMap<Version, ItemStatus>;

#[derive(Debug, Clone)]
pub(crate) struct ContainerVersion {
    /// Indicates that the container version is deprecated.
    pub(crate) deprecated: bool,

    /// Indicates that the generation of `From<OLD> for NEW` should be skipped.
    pub(crate) skip_from: bool,

    /// A validated Kubernetes API version.
    pub(crate) inner: Version,

    /// The ident of the container.
    pub(crate) ident: Ident,
}

impl From<&ContainerAttributes> for Vec<ContainerVersion> {
    fn from(attributes: &ContainerAttributes) -> Self {
        attributes
            .versions
            .iter()
            .map(|v| ContainerVersion {
                skip_from: v.skip.as_ref().map_or(false, |s| s.from.is_present()),
                ident: Ident::new(&v.name.to_string(), Span::call_site()),
                deprecated: v.deprecated.is_present(),
                inner: v.name,
            })
            .collect()
    }
}

/// Returns the container ident used in [`From`] implementations.
pub(crate) fn format_container_from_ident(ident: &Ident) -> Ident {
    format_ident!("__sv_{ident}", ident = ident.to_string().to_lowercase())
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
