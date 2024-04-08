use darling::{util::SpannedValue, Error, FromField, FromMeta};
use syn::Ident;

#[derive(Debug, FromField)]
#[darling(
    attributes(versioned),
    forward_attrs(allow, doc, cfg, serde),
    and_then = FieldAttributes::validate
)]
pub(crate) struct FieldAttributes {
    added: Option<SpannedValue<AddedAttributes>>,
    renamed: Option<SpannedValue<RenamedAttributes>>,
    deprecated: Option<SpannedValue<DeprecatedAttributes>>,

    ident: Option<Ident>,

    #[darling(skip)]
    _action: FieldAction,
}

impl FieldAttributes {
    fn validate(self) -> darling::Result<Self> {
        match (&self.added, &self.renamed, &self.deprecated) {
            (Some(_), Some(_), Some(_)) => {
                return Err(Error::custom(
                    "cannot specify fields `added`, `renamed`, and `deprecated` at the same time",
                )
                .with_span(&self.ident.unwrap().span()))
            }
            (Some(_), Some(_), None) => {
                return Err(Error::custom(
                    "cannot specify fields `added` and `renamed` at the same time",
                )
                .with_span(&self.ident.unwrap().span()))
            }
            (Some(_), None, Some(_)) => {
                return Err(Error::custom(
                    "cannot specify fields `added` and `deprecated` at the same time",
                )
                .with_span(&self.ident.unwrap().span()))
            }
            _ => (),
        }

        Ok(self)
    }
}

#[derive(Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    #[darling(rename = "since")]
    _since: SpannedValue<String>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    _since: SpannedValue<String>,
    _to: SpannedValue<String>,
}

#[derive(Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    _since: SpannedValue<String>,
}

#[derive(Debug, Default)]
pub(crate) enum FieldAction {
    #[default]
    Added,
    // Renamed,
    // Deprecated,
}
