use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Data, DataEnum, DataStruct, DeriveInput, Error, Ident, Result};

use crate::attrs::container::ContainerAttributes;

pub(crate) mod version;

pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
    // Extract container attributes
    let attributes = ContainerAttributes::from_derive_input(&input)?;

    // Validate container shape
    let expanded = match input.data {
        Data::Struct(data) => expand_struct(input.ident, data, attributes)?,
        Data::Enum(data) => expand_enum(input.ident, data, attributes)?,
        Data::Union(_) => {
            return Err(Error::new(
                input.span(),
                "derive macro `Versioned` only supports structs and enums",
            ))
        }
    };

    Ok(quote! {
        #expanded
    })
}

pub(crate) fn expand_struct(
    ident: Ident,
    data: DataStruct,
    attributes: ContainerAttributes,
) -> Result<TokenStream> {
    Ok(quote!())
}

pub(crate) fn expand_enum(
    ident: Ident,
    data: DataEnum,
    attributes: ContainerAttributes,
) -> Result<TokenStream> {
    Ok(quote!())
}
