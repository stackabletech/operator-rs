use proc_macro2::TokenStream;
use quote::quote;
use syn::Visibility;

use crate::codegen::common::VersionDefinition;

pub(crate) fn generate_module(
    version: &VersionDefinition,
    visibility: &Visibility,
    content: TokenStream,
) -> TokenStream {
    let version_ident = &version.ident;

    let deprecated_attribute = version.deprecated.then(|| {
        let deprecated_note = format!("Version {version_ident} is deprecated");

        quote! {
            #[deprecated = #deprecated_note]
        }
    });

    quote! {
        #[automatically_derived]
        #deprecated_attribute
        #visibility mod #version_ident {
            use super::*;

            #content
        }
    }
}
