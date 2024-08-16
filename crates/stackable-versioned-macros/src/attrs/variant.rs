use convert_case::{Case, Casing};
use darling::{Error, FromVariant};
use syn::Ident;

use crate::attrs::common::{ItemAttributes, ItemType};

/// This struct describes all available variant attributes, as well as the
/// variant name to display better diagnostics.
///
/// Data stored in this struct is validated using darling's `and_then` attribute.
/// During darlings validation, it is not possible to validate that action
/// versions match up with declared versions on the container. This validation
/// can be done using the associated [`FieldAttributes::validate_versions`][1]
/// function.
///
/// Rules shared across fields and variants can be found [here][2].
///
/// [1]: crate::attrs::common::ValidateVersions::validate_versions
/// [2]: crate::attrs::common::ItemAttributes
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
