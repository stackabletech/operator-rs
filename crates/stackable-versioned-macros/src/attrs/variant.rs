use convert_case::{Case, Casing};
use darling::{Error, FromVariant};
use syn::{Ident, Variant};

use crate::attrs::common::{ContainerAttributes, ItemAttributes, ItemType};

#[derive(Debug, FromVariant)]
#[darling(
    attributes(versioned),
    forward_attrs(allow, doc, cfg, serde),
    and_then = VariantAttributes::validate
)]
pub(crate) struct VariantAttributes {
    #[darling(flatten)]
    pub(crate) common: ItemAttributes,

    // The ident (automatically extracted by darling) cannot be moved into the
    // shared item attributes because for struct fields, the type is
    // `Option<Ident>`, while for enum variants, the type is `Ident`.
    pub(crate) ident: Ident,
}

impl VariantAttributes {
    // NOTE (@Techassi): Ideally, these validations should be moved to the
    // ItemAttributes impl, because common validation like action combinations
    // and action order can be validated without taking the type of attribute
    // into account (field vs variant). However, we would loose access to the
    // field / variant ident and as such, cannot display the error directly on
    // the affected field / variant. This is a significant decrease in DX.
    // See https://github.com/TedDriggs/darling/discussions/294

    /// This associated function is called by darling (see and_then attribute)
    /// after it successfully parsed the attribute. This allows custom
    /// validation of the attribute which extends the validation already in
    /// place by darling.
    ///
    /// Internally, it calls out to other specialized validation functions.
    fn validate(self) -> Result<Self, Error> {
        let mut errors = Error::accumulator();

        errors.handle(self.common.validate(&self.ident, &ItemType::Variant));

        // Validate names of renames
        if !self
            .common
            .renames
            .iter()
            .all(|r| r.from.is_case(Case::Pascal))
        {
            errors.push(Error::custom("renamed variants must use PascalCase"));
        }

        errors.finish()?;
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

        if let Some(added) = &self.common.added {
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

        for rename in &*self.common.renames {
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

        if let Some(deprecated) = &self.common.deprecated {
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
