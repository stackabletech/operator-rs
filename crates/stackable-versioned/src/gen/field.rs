use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::Field;

use crate::attrs::field::FieldAction;

pub(crate) struct VersionedField {
    // TODO (@Techassi): There can be multiple actions for one field (in
    // different versions). Add support for that here.
    pub(crate) action: FieldAction,
    pub(crate) inner: Field,
}

impl ToTokens for VersionedField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match &self.action {
            FieldAction::Renamed(renamed) => {
                let field_name = format_ident!("{}", *renamed.to);
                let field_type = &self.inner.ty;

                tokens.extend(quote! {
                    pub #field_name: #field_type,
                })
            }
            FieldAction::Deprecated(deprecated) => {
                // TODO (@Techassi): Is it save to unwrap here?
                let field_name = format_ident!(
                    "deprecated_{}",
                    &self.inner.ident.as_ref().expect("field must have a name")
                );
                let field_type = &self.inner.ty;

                let deprecation_note = format!(
                    "{} (was deprecated in {})",
                    &*deprecated.note, &*deprecated.since
                );

                tokens.extend(quote! {
                    #[deprecated = #deprecation_note]
                    pub #field_name: #field_type,
                })
            }
            FieldAction::Added(_) | FieldAction::None => {
                let field_name = &self.inner.ident;
                let field_type = &self.inner.ty;

                tokens.extend(quote! {
                    pub #field_name: #field_type,
                })
            }
        }
    }
}

impl VersionedField {
    pub(crate) fn new(field: Field, action: FieldAction) -> Self {
        Self {
            inner: field,
            action,
        }
    }
}
