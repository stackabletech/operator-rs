use darling::{Error, FromField, FromMeta, Result, util::Flag};
use syn::{Attribute, Ident};

use crate::{
    attrs::item::CommonItemAttributes,
    codegen::{VersionDefinition, item::FieldIdents},
};

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

    /// Indicates that this field's type is a nested sub struct. The indicator
    /// is needed to let the macro know to generate conversion code with support
    /// for tracking across struct boundaries.
    pub nested: Flag,

    /// Provide a hint if a field is wrapped in either `Option` or `Vec` to
    /// generate correct code in the `From` impl blocks.
    pub hint: Option<Hint>,
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

        self.common
            .validate(FieldIdents::from(ident), &self.attrs)?;

        Ok(self)
    }

    pub fn validate_versions(&self, versions: &[VersionDefinition]) -> Result<()> {
        self.common.validate_versions(versions)
    }

    pub fn validate_nested_flag(&self, experimental_conversion_tracking: bool) -> Result<()> {
        if self.nested.is_present() && !experimental_conversion_tracking {
            return Err(
                Error::custom("the `nested` argument can only be used if the module-level `experimental_conversion_tracking` flag is set")
                    .with_span(&self.nested.span())
            );
        }

        Ok(())
    }
}

/// Supported field hints.
#[derive(Debug, FromMeta)]
pub enum Hint {
    Option,
    Vec,
}
