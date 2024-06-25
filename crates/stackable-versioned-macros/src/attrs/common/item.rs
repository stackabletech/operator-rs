use darling::{util::SpannedValue, Error, FromMeta};
use k8s_version::Version;
use proc_macro2::Span;
use syn::Path;

/// These attributes are meant to be used in super structs, which add
/// [`Field`](syn::Field) or [`Variant`](syn::Variant) specific attributes via
/// darling's flatten feature. This struct only provides shared attributes.
#[derive(Debug, FromMeta)]
#[darling(and_then = ItemAttributes::validate)]
pub(crate) struct ItemAttributes {
    /// This parses the `added` attribute on items (fields or variants). It can
    /// only be present at most once.
    pub(crate) added: Option<AddedAttributes>,

    /// This parses the `renamed` attribute on items (fields or variants). It
    /// can be present 0..n times.
    #[darling(multiple, rename = "renamed")]
    pub(crate) renames: Vec<RenamedAttributes>,

    /// This parses the `deprecated` attribute on items (fields or variants). It
    /// can only be present at most once.
    pub(crate) deprecated: Option<DeprecatedAttributes>,
}

impl ItemAttributes {
    fn validate(self) -> Result<Self, Error> {
        // Validate deprecated options

        // TODO (@Techassi): Make the field 'note' optional, because in the
        // future, the macro will generate parts of the deprecation note
        // automatically. The user-provided note will then be appended to the
        // auto-generated one.

        if let Some(deprecated) = &self.deprecated {
            if deprecated.note.is_empty() {
                return Err(Error::custom("deprecation note must not be empty")
                    .with_span(&deprecated.note.span()));
            }
        }

        Ok(self)
    }
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct AddedAttributes {
    pub(crate) since: SpannedValue<Version>,

    #[darling(rename = "default", default = "default_default_fn")]
    pub(crate) default_fn: SpannedValue<Path>,
}

fn default_default_fn() -> SpannedValue<Path> {
    SpannedValue::new(
        syn::parse_str("std::default::Default::default").expect("internal error: path must parse"),
        Span::call_site(),
    )
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct RenamedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) from: SpannedValue<String>,
}

#[derive(Clone, Debug, FromMeta)]
pub(crate) struct DeprecatedAttributes {
    pub(crate) since: SpannedValue<Version>,
    pub(crate) note: SpannedValue<String>,
}
