use convert_case::{Case, Casing};
use darling::{Error, FromVariant};
use syn::Ident;

use crate::attrs::common::{ItemAttributes, ItemType};

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
}
