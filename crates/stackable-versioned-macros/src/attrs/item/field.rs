use darling::{FromField, Result};
use syn::{Attribute, Ident};

use crate::{attrs::item::CommonItemAttributes, codegen::VersionDefinition, utils::FieldIdent};

/// This struct describes all available field attributes, as well as the field
/// name to display better diagnostics.
///
/// Data stored in this struct is validated using darling's `and_then` attribute.
/// During darlings validation, it is not possible to validate that action
/// versions match up with declared versions on the container. This validation
/// can be done using the associated [`FieldAttributes::validate_versions`][1]
/// function.
///
/// Rules shared across fields and variants can be found [here][2].
///
/// [1]: crate::attrs::item::FieldAttributes::validate_versions
/// [2]: crate::attrs::item::CommonItemAttributes
#[derive(Debug, FromField)]
#[darling(
    attributes(versioned),
    forward_attrs,
    and_then = FieldAttributes::validate
)]
pub struct FieldAttributes {
    #[darling(flatten)]
    pub common: CommonItemAttributes,

    // The ident (automatically extracted by darling) cannot be moved into the
    // shared item attributes because for struct fields, the type is
    // `Option<Ident>`, while for enum variants, the type is `Ident`.
    pub ident: Option<Ident>,

    // This must be named `attrs` for darling to populate it accordingly, and
    // cannot live in common because Vec<Attribute> is not implemented for
    // FromMeta.
    /// The original attributes for the field.
    pub attrs: Vec<Attribute>,
}

impl FieldAttributes {
    /// This associated function is called by darling (see and_then attribute)
    /// after it successfully parsed the attribute. This allows custom
    /// validation of the attribute which extends the validation already in
    /// place by darling.
    ///
    /// Internally, it calls out to other specialized validation functions.
    fn validate(self) -> Result<Self> {
        let ident = self
            .ident
            .as_ref()
            .expect("internal error: field must have an ident")
            .clone();

        self.common.validate(FieldIdent::from(ident), &self.attrs)?;
        Ok(self)
    }

    pub fn validate_versions(&self, versions: &[VersionDefinition]) -> Result<()> {
        self.common.validate_versions(versions)
    }
}
