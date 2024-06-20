use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Data, DeriveInput, Error, Result};

use crate::{
    attrs::container::ContainerAttributes,
    gen::{common::Container, venum::VersionedEnum, vstruct::VersionedStruct},
};

pub(crate) mod common;
pub(crate) mod neighbors;
pub(crate) mod venum;
pub(crate) mod vstruct;

// NOTE (@Techassi): This derive macro cannot handle multiple structs / enums
// to be versioned within the same file. This is because we cannot declare
// modules more than once (They will not be merged, like impl blocks for
// example). This leads to collisions if there are multiple structs / enums
// which declare the same version. This could maybe be solved by using an
// attribute macro applied to a module with all struct / enums declared in said
// module. This would allow us to generate all versioned structs and enums in
// a single sweep and put them into the appropriate module.

// TODO (@Techassi): Think about how we can handle nested structs / enums which
// are also versioned.

pub(crate) fn expand(attributes: ContainerAttributes, input: DeriveInput) -> Result<TokenStream> {
    let expanded = match input.data {
        Data::Struct(data) => {
            VersionedStruct::new(input.ident, data, attributes)?.generate_tokens()
        }
        Data::Enum(data) => VersionedEnum::new(input.ident, data, attributes)?.generate_tokens(),
        _ => {
            return Err(Error::new(
                input.span(),
                "attribute macro `versioned` only supports structs and enums",
            ))
        }
    };

    Ok(expanded)
}
