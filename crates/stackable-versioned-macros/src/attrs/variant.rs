// TODO (@Techassi): Think about what can be moved into a common impl for both
// fields and variants.

use darling::{Error, FromVariant};
use syn::{Ident, Variant};

use crate::attrs::common::{
    AddedAttributes, ContainerAttributes, DeprecatedAttributes, RenamedAttributes,
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
        container_attrs: &ContainerAttributes,
        variant: &Variant,
    ) -> Result<(), Error> {
        // NOTE (@Techassi): Can we maybe optimize this a little?
        // TODO (@Techassi): Unify this with the field impl, e.g. by introducing
        // a T: Spanned bound for the second function parameter.
        let mut errors = Error::accumulator();

        if let Some(added) = &self.added {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *added.since)
            {
                errors.push(Error::custom(
                   "variant action `added` uses version which was not declared via #[versioned(version)]")
                   .with_span(&variant.ident)
               );
            }
        }

        for rename in &self.renames {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *rename.since)
            {
                errors.push(
                   Error::custom("variant action `renamed` uses version which was not declared via #[versioned(version)]")
                   .with_span(&variant.ident)
               );
            }
        }

        if let Some(deprecated) = &self.deprecated {
            if !container_attrs
                .versions
                .iter()
                .any(|v| v.name == *deprecated.since)
            {
                errors.push(Error::custom(
                   "variant action `deprecated` uses version which was not declared via #[versioned(version)]")
                   .with_span(&variant.ident)
               );
            }
        }

        errors.finish()?;
        Ok(())
    }
}
