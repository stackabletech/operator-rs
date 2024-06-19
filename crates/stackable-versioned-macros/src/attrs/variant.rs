// TODO (@Techassi): Think about what can be moved into a common impl for both
// fields and variants.

use darling::{Error, FromVariant};
use syn::{Ident, Variant};

use crate::attrs::{
    common::{AddedAttributes, DeprecatedAttributes, RenamedAttributes},
    container::ContainerAttributes,
};

#[derive(Debug, FromVariant)]
#[darling(
    attributes(versioned),
    forward_attrs(allow, doc, cfg, serde),
    and_then = VariantAttributes::validate
)]
pub(crate) struct VariantAttributes {
    pub(crate) ident: Ident,
    pub(crate) added: Option<AddedAttributes>,

    #[darling(multiple, rename = "renamed")]
    pub(crate) renames: Vec<RenamedAttributes>,

    pub(crate) deprecated: Option<DeprecatedAttributes>,
}

impl VariantAttributes {
    pub(crate) fn validate(self) -> Result<Self, Error> {
        Ok(self)
    }

    pub(crate) fn validate_versions(
        &self,
        attributes: &ContainerAttributes,
        variant: &Variant,
    ) -> Result<(), Error> {
        todo!()
    }
}
