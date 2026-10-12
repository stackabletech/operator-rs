use std::{collections::BTreeMap, ops::Bound};

use darling::util::IdentString;
use k8s_version::Version;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Meta, Path, Type};

use crate::codegen::changes::Neighbors as _;

mod field;
pub use field::*;

mod variant;
pub use variant::*;

/// Generates the attributes of an item (field or variant) for the provided `version`.
///
/// If the docs of the item are changed in a later version (via `from_docs`), the original doc
/// comments are replaced by the docs which are valid in `version`.
pub fn generate_attributes(
    original_attributes: &[Attribute],
    previous_docs: &BTreeMap<Version, Vec<String>>,
    version: &Version,
) -> TokenStream {
    // The docs valid in this version are the previous docs of the closest change after this
    // version. If there is no such change, the original docs are still valid.
    let Some((_, docs)) = previous_docs.up_bound(Bound::Excluded(version)) else {
        return quote! { #(#original_attributes)* };
    };

    let attributes = original_attributes.iter().filter(|attribute| {
        !matches!(&attribute.meta, Meta::NameValue(name_value) if name_value.path.is_ident("doc"))
    });

    quote! {
        #(#[doc = #docs])*
        #(#attributes)*
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ItemStatus {
    Addition {
        ident: IdentString,
        default_fn: Path,
        // NOTE (@Techassi): We need to carry idents and type information in
        // nearly every status. Ideally, we would store this in separate maps.
        ty: Box<Type>,
    },
    Change {
        downgrade_with: Option<Path>,
        upgrade_with: Option<Path>,
        from_ident: IdentString,
        to_ident: IdentString,
        from_type: Box<Type>,
        to_type: Box<Type>,
    },
    Deprecation {
        previous_ident: IdentString,
        note: Option<String>,
        ident: IdentString,
    },
    NoChange {
        previously_deprecated: bool,
        ident: IdentString,
        ty: Box<Type>,
    },
    NotPresent,
}

impl ItemStatus {
    pub fn get_ident(&self) -> &IdentString {
        match &self {
            Self::Addition { ident, .. }
            | Self::Change {
                to_ident: ident, ..
            }
            | Self::Deprecation { ident, .. }
            | Self::NoChange { ident, .. } => ident,
            Self::NotPresent => unreachable!("ItemStatus::NotPresent does not have an ident"),
        }
    }
}
