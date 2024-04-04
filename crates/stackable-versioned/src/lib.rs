use proc_macro::TokenStream;
use syn::{DeriveInput, Error};

mod gen;

#[proc_macro_derive(Versioned, attributes(versioned))]
pub fn versioned_macro_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    gen::expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
