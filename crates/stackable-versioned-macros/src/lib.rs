use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use syn::{DeriveInput, Error};

use crate::attrs::container::ContainerAttributes;

mod attrs;
mod consts;
mod gen;

#[proc_macro_attribute]
pub fn versioned(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = match NestedMeta::parse_meta_list(attrs.into()) {
        Ok(attrs) => match ContainerAttributes::from_list(&attrs) {
            Ok(attrs) => attrs,
            Err(err) => return err.write_errors().into(),
        },
        Err(err) => return darling::Error::from(err).write_errors().into(),
    };

    // NOTE (@Techassi): For now, we can just use the DeriveInput type here,
    // because we only support structs (and eventually enums) to be versioned.
    // In the future - if we decide to support modules - this requires
    // adjustments to also support modules. One possible solution might be to
    // use an enum with two variants: Container(DeriveInput) and
    // Module(ItemMod).
    let input = syn::parse_macro_input!(input as DeriveInput);

    gen::expand(attrs, input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
