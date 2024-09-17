use std::collections::BTreeMap;

use k8s_version::Version;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
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

    /// Store additional doc-comment lines for this version.
    pub(crate) docs: Docs,
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
                docs: v.doc.clone().into(),
                inner: v.name,
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Docs(Vec<String>);

impl From<Option<String>> for Docs {
    fn from(doc: Option<String>) -> Self {
        let lines = if let Some(doc) = doc {
            doc
                // Trim the leading and trailing whitespace, deleting
                // superfluous empty lines.
                .trim()
                .lines()
                // Trim the leading and trailing whitespace on each line that
                // can be introduced when the developer indents multi-line
                // comments.
                .map(|line| line.trim().into())
                .collect()
        } else {
            Vec::new()
        };

        Self(lines)
    }
}

impl ToTokens for Docs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for (index, line) in self.0.iter().enumerate() {
            if index == 0 {
                // Prepend an empty line to clearly separate the version/action
                // specific docs.
                tokens.extend(quote! {
                    #[doc = ""]
                })
            }

            tokens.extend(quote! {
                #[doc = #line]
            })
        }
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
