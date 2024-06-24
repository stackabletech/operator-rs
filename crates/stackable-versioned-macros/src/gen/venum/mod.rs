use std::ops::Deref;

use darling::FromVariant;
use itertools::Itertools;
use proc_macro2::TokenStream;
use syn::{DataEnum, Error, Ident};

use crate::{
    attrs::{container::ContainerAttributes, variant::VariantAttributes},
    gen::{
        common::{format_container_from_ident, Container, Item, VersionedContainer},
        venum::variant::VersionedVariant,
    },
};

mod variant;

#[derive(Debug)]
pub(crate) struct VersionedEnum(VersionedContainer<VersionedVariant>);

impl Container<DataEnum, VersionedVariant> for VersionedEnum {
    fn new(ident: Ident, data: DataEnum, attributes: ContainerAttributes) -> syn::Result<Self> {
        // Convert the raw version attributes into a container version.
        let versions: Vec<_> = (&attributes).into();

        // Extract the field attributes for every field from the raw token
        // stream and also validate that each field action version uses a
        // version declared by the container attribute.
        let mut items = Vec::new();

        for variant in data.variants {
            let attrs = VariantAttributes::from_variant(&variant)?;
            attrs.validate_versions(&attributes, &variant)?;

            let mut versioned_field = VersionedVariant::new(variant, attrs);
            versioned_field.insert_container_versions(&versions);
            items.push(versioned_field);
        }

        // Check for field ident collisions
        for version in &versions {
            // Collect the idents of all fields for a single version and then
            // ensure that all idents are unique. If they are not, return an
            // error.

            // TODO (@Techassi): Report which field(s) use a duplicate ident and
            // also hint what can be done to fix it based on the field action /
            // status.

            if !items.iter().map(|f| f.get_ident(version)).all_unique() {
                return Err(Error::new(
                    ident.span(),
                    format!("struct contains renamed fields which collide with other fields in version {version}", version = version.inner),
                ));
            }
        }

        let from_ident = format_container_from_ident(&ident);

        Ok(Self(VersionedContainer {
            skip_from: attributes
                .options
                .skip
                .map_or(false, |s| s.from.is_present()),
            from_ident,
            versions,
            items,
            ident,
        }))
    }

    fn generate_tokens(&self) -> TokenStream {
        todo!()
    }
}

impl Deref for VersionedEnum {
    type Target = VersionedContainer<VersionedVariant>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
