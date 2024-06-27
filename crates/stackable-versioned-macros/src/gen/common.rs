use std::{collections::BTreeMap, ops::Deref};

use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use quote::format_ident;
use syn::{Ident, Path};

use crate::{
    attrs::common::ContainerAttributes,
    consts::{DEPRECATED_FIELD_PREFIX, DEPRECATED_VARIANT_PREFIX},
};

pub(crate) type VersionChain = BTreeMap<Version, ItemStatus>;

#[derive(Debug, Clone)]
pub(crate) struct ContainerVersion {
    pub(crate) deprecated: bool,
    pub(crate) skip_from: bool,
    pub(crate) inner: Version,
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

pub(crate) trait Container<D, I>
where
    Self: Sized + Deref<Target = VersionedContainer<I>>,
{
    fn new(ident: Ident, data: D, attributes: ContainerAttributes) -> syn::Result<Self>;

    /// This generates the complete code for a single versioned container.
    ///
    /// Internally, it will create a module for each declared version which
    /// contains the container with the appropriate items (fields or variants)
    ///  Additionally, it generates `From` implementations, which enable
    /// conversion from an older to a newer version.
    fn generate_tokens(&self) -> TokenStream;
}

#[derive(Debug)]
pub(crate) struct VersionedContainer<I> {
    pub(crate) versions: Vec<ContainerVersion>,
    pub(crate) items: Vec<I>,
    pub(crate) ident: Ident,

    pub(crate) from_ident: Ident,
    pub(crate) skip_from: bool,
}

#[derive(Debug)]
pub(crate) enum ItemStatus {
    Added {
        ident: Ident,
        default_fn: Path,
    },
    Renamed {
        from: Ident,
        to: Ident,
    },
    Deprecated {
        previous_ident: Ident,
        ident: Ident,
        note: String,
    },
    NoChange(Ident),
    NotPresent,
}

impl ItemStatus {
    pub(crate) fn get_ident(&self) -> Option<&Ident> {
        match &self {
            ItemStatus::Added { ident, .. } => Some(ident),
            ItemStatus::Renamed { to, .. } => Some(to),
            ItemStatus::Deprecated { ident, .. } => Some(ident),
            ItemStatus::NoChange(ident) => Some(ident),
            ItemStatus::NotPresent => None,
        }
    }
}

/// Returns the container ident used in [`From`] implementations.
pub(crate) fn format_container_from_ident(ident: &Ident) -> Ident {
    format_ident!("__sv_{ident}", ident = ident.to_string().to_lowercase())
}

/// Removes the deprecated prefix from field ident.
pub(crate) fn remove_deprecated_field_prefix(ident: &Ident) -> Ident {
    remove_ident_prefix(ident, DEPRECATED_FIELD_PREFIX)
}

pub(crate) fn remove_deprecated_variant_prefix(ident: &Ident) -> Ident {
    remove_ident_prefix(ident, DEPRECATED_VARIANT_PREFIX)
}

pub(crate) fn remove_ident_prefix(ident: &Ident, prefix: &str) -> Ident {
    format_ident!("{}", ident.to_string().trim_start_matches(prefix))
}
