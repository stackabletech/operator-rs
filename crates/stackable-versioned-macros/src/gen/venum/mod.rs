use std::borrow::Borrow;

use darling::FromVariant;
use itertools::Itertools;
use proc_macro2::TokenStream;
use syn::{DataEnum, Error, Ident, Result};

use crate::{
    attrs::{container::ContainerAttributes, variant::VariantAttributes},
    gen::{common::ContainerVersion, venum::variant::VersionedVariant},
};

mod variant;

#[derive(Debug)]
pub(crate) struct VersionedEnum {
    /// The ident, or name, of the versioned enum.
    pub(crate) ident: Ident,

    /// List of declared versions for this enum. Each version, except the
    /// latest, generates a definition with appropriate fields.
    pub(crate) versions: Vec<ContainerVersion>,

    /// List of variants defined in the base enum. How, and if, a variant should
    /// generate code, is decided by the currently generated version.
    pub(crate) variants: Vec<VersionedVariant>,

    /// The name of the enum used in `From` implementations.
    pub(crate) from_ident: Ident,
    pub(crate) skip_from: bool,
}

impl VersionedEnum {
    pub(crate) fn new(
        ident: Ident,
        data: DataEnum,
        attributes: ContainerAttributes,
    ) -> Result<Self> {
        let versions: Vec<ContainerVersion> = attributes.borrow().into();
        let mut variants = Vec::new();

        for variant in data.variants {
            let attrs = VariantAttributes::from_variant(&variant)?;
            attrs.validate_versions(&attributes, &variant)?;

            let mut versioned_variant = VersionedVariant::new();
            versioned_variant.insert_container_versions(&versions);
            variants.push(versioned_variant);
        }

        for version in &versions {
            if !variants.iter().map(|v| v.get_ident(version)).all_unique() {
                return Err(Error::new(
                    ident.span(),
                    format!("enum contains renamed variant which collide with other variant in version {version}", version = version.inner),
                ));
            }
        }

        todo!()
    }

    pub(crate) fn generate_tokens(&self) -> TokenStream {
        todo!()
    }
}
