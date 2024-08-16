use darling::{Error, FromField};
use syn::Ident;

use crate::attrs::common::{ItemAttributes, ItemType};

/// This struct describes all available field attributes, as well as the field
/// name to display better diagnostics.
///
/// Data stored in this struct is validated using darling's `and_then` attribute.
/// During darlings validation, it is not possible to validate that action
/// versions match up with declared versions on the container. This validation
/// can be done using the associated [`FieldAttributes::validate_versions`]
/// function.
///
/// ### Field Rules
///
/// - A field can only ever be added once at most. A field not marked as 'added'
///   is part of the struct in every version until renamed or deprecated.
/// - A field can be renamed many times. That's why renames are stored in a
///   [`Vec`].
/// - A field can only be deprecated once. A field not marked as 'deprecated'
///   will be included up until the latest version.
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
        self.common
            .validate(self.ident.as_ref().unwrap(), &ItemType::Field)?;

        Ok(self)
    }
}
