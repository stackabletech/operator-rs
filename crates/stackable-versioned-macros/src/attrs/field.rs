use darling::{Error, FromField};
use syn::Ident;

use crate::attrs::common::{ItemAttributes, ItemType};

/// This struct describes all available field attributes, as well as the field
/// name to display better diagnostics.
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
#[derive(Debug, FromField)]
#[darling(
    attributes(versioned),
    forward_attrs(allow, doc, cfg, serde),
    and_then = FieldAttributes::validate
)]
pub(crate) struct FieldAttributes {
    #[darling(flatten)]
    pub(crate) common: ItemAttributes,

    // The ident (automatically extracted by darling) cannot be moved into the
    // shared item attributes because for struct fields, the type is
    // `Option<Ident>`, while for enum variants, the type is `Ident`.
    pub(crate) ident: Option<Ident>,
}

impl FieldAttributes {
    /// This associated function is called by darling (see and_then attribute)
    /// after it successfully parsed the attribute. This allows custom
    /// validation of the attribute which extends the validation already in
    /// place by darling.
    ///
    /// Internally, it calls out to other specialized validation functions.
    fn validate(self) -> Result<Self, Error> {
        let ident = self
            .ident
            .as_ref()
            .expect("internal error: field must have an ident");
        self.common.validate(ident, &ItemType::Field)?;

        Ok(self)
    }
}
