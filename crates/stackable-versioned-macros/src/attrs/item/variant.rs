use convert_case::{Case, Casing};
use darling::{Error, FromVariant, Result};
use syn::{Attribute, Ident};

use crate::{attrs::item::CommonItemAttributes, codegen::VersionDefinition, utils::VariantIdent};

/// This struct describes all available variant attributes, as well as the
/// variant name to display better diagnostics.
///
/// Data stored in this struct is validated using darling's `and_then` attribute.
/// During darlings validation, it is not possible to validate that action
/// versions match up with declared versions on the container. This validation
/// can be done using the associated [`ValidateVersions::validate_versions`][1]
/// function.
///
/// Rules shared across fields and variants can be found [here][2].
///
/// [1]: crate::attrs::common::ValidateVersions::validate_versions
/// [2]: crate::attrs::common::ItemAttributes
#[derive(Debug, FromVariant)]
#[darling(
    attributes(versioned),
    forward_attrs,
    and_then = VariantAttributes::validate
)]
pub(crate) struct VariantAttributes {
    #[darling(flatten)]
    pub(crate) common: CommonItemAttributes,

    // The ident (automatically extracted by darling) cannot be moved into the
    // shared item attributes because for struct fields, the type is
    // `Option<Ident>`, while for enum variants, the type is `Ident`.
    pub(crate) ident: Ident,

    // This must be named `attrs` for darling to populate it accordingly, and
    // cannot live in common because Vec<Attribute> is not implemented for
    // FromMeta.
    /// The original attributes for the field.
    pub(crate) attrs: Vec<Attribute>,
}

impl VariantAttributes {
    /// This associated function is called by darling (see and_then attribute)
    /// after it successfully parsed the attribute. This allows custom
    /// validation of the attribute which extends the validation already in
    /// place by darling.
    ///
    /// Internally, it calls out to other specialized validation functions.
    fn validate(self) -> Result<Self> {
        let mut errors = Error::accumulator();

        errors.handle(
            self.common
                .validate(VariantIdent::from(self.ident.clone()), &self.attrs),
        );

        // Validate names of renames
        for change in &self.common.changes {
            if let Some(from_name) = &change.from_name {
                if !from_name.is_case(Case::Pascal) {
                    errors.push(
                        Error::custom("renamed variant must use PascalCase")
                            .with_span(&from_name.span()),
                    )
                }
            }
        }

        errors.finish_with(self)
    }

    pub(crate) fn validate_versions(&self, versions: &[VersionDefinition]) -> Result<()> {
        self.common.validate_versions(versions)
    }
}
