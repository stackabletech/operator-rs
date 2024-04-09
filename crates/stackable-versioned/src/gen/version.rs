use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::Ident;

use crate::gen::field::VersionedField;

// TODO (@Techassi): Remove allow attribute
#[allow(dead_code)]
pub(crate) struct Version {
    pub(crate) fields: Vec<VersionedField>,
    pub(crate) container_ident: Ident,
    pub(crate) module: Ident,
    pub(crate) name: String,
}

impl ToTokens for Version {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let module_name = &self.module;
        let container_ident = &self.container_ident;

        tokens.extend(quote! {
            #[automatically_derived]
            pub mod #module_name {
                pub struct #container_ident {}
            }
        })
    }
}

impl Version {
    pub(crate) fn new(container_ident: Ident, version: String) -> Self {
        Self {
            module: format_ident!("{}", version.to_lowercase()),
            fields: Vec::new(),
            container_ident,
            name: version,
        }
    }
}
