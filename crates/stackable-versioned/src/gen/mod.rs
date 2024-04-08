use darling::{FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Data, DataEnum, DataStruct, DeriveInput, Error, Ident, Result};

use crate::attrs::{container::ContainerAttributes, field::FieldAttributes};

pub(crate) mod version;

pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
    // Extract container attributes
    let attributes = ContainerAttributes::from_derive_input(&input)?;

    // Validate container shape
    let expanded = match input.data {
        Data::Struct(data) => expand_struct(&input.ident, data, attributes)?,
        Data::Enum(data) => expand_enum(&input.ident, data, attributes)?,
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
    _ident: &Ident,
    data: DataStruct,
    _attributes: ContainerAttributes,
) -> Result<TokenStream> {
    // Loop over each specified version and collect fields added, renamed
    // and deprecated for that version.
    for field in data.fields {
        let _field_aatributes = FieldAttributes::from_field(&field)?;
    }

    Ok(quote!())
}

pub(crate) fn expand_enum(
    _ident: &Ident,
    _data: DataEnum,
    _attributes: ContainerAttributes,
) -> Result<TokenStream> {
    Ok(quote!())
}
