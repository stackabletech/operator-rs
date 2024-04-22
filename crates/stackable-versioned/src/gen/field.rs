use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Field};

use crate::{
    attrs::field::FieldAction,
    gen::{version::ContainerVersion, ToTokensExt},
};

pub(crate) struct VersionedField {
    // TODO (@Techassi): There can be multiple actions for one field (in
    // different versions). Add support for that here.
    pub(crate) action: FieldAction,
    pub(crate) inner: Field,
}

impl ToTokensExt for VersionedField {
    fn to_tokens_for_version(&self, version: &ContainerVersion) -> Option<TokenStream> {
        match &self.action {
            FieldAction::Added(added) => {
                // Skip generating the field, if the current generated
                // version appears before the field action version.
                if version.inner < *added.since {
                    return None;
                }

                let field_name = &self.inner.ident;
                let field_type = &self.inner.ty;
                let doc = format!(" Added since `{}`.", *added.since);

                // TODO (@Techassi): Also forward other attributes
                let doc_attrs: Vec<&Attribute> = self
                    .inner
                    .attrs
                    .iter()
                    .filter(|a| a.path().is_ident("doc"))
                    .collect();

                Some(quote! {
                    #(#doc_attrs)*
                    #[doc = ""]
                    #[doc = #doc]
                    pub #field_name: #field_type,
                })
            }
            FieldAction::Renamed(renamed) => {
                if version.inner < *renamed.since {
                    // Use the original name for versions before the field action
                    // version.
                    let field_name = format_ident!("{}", *renamed.from);
                    let field_type = &self.inner.ty;

                    Some(quote! {
                        pub #field_name: #field_type,
                    })
                } else {
                    // Use the new name for versions equal or greater than the
                    // field action version
                    let field_name = &self.inner.ident;
                    let field_type = &self.inner.ty;

                    Some(quote! {
                        pub #field_name: #field_type,
                    })
                }
            }
            FieldAction::Deprecated(_) => todo!(),
            FieldAction::None => {
                // Generate fields without any attributes in every version.
                let field_name = &self.inner.ident;
                let field_type = &self.inner.ty;

                Some(quote! {
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
