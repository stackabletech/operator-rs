use syn::{Field, Ident};

pub(crate) struct Version {
    struct_ident: Ident,
    version: String,

    deprecated: Vec<Field>,
    renamed: Vec<Field>,
    added: Vec<Field>,

    fields: Vec<Field>,
}
