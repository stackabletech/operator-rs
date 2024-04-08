use syn::{Field, Ident};

// TODO (@Techassi): Remove allow attribute
#[allow(dead_code)]
pub(crate) struct Version {
    struct_ident: Ident,
    version: String,

    deprecated: Vec<Field>,
    renamed: Vec<Field>,
    added: Vec<Field>,

    fields: Vec<Field>,
}
