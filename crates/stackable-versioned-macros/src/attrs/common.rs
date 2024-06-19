use darling::{util::SpannedValue, FromMeta};
use k8s_version::Version;
use proc_macro2::Span;
use syn::Path;

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
