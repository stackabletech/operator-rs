use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::Field;

use crate::attrs::field::FieldAction;

pub(crate) struct VersionedField {
    // TODO (@Techassi): There can be multiple actions for one field (in
    //different versions). Add support for that here.
    pub(crate) _action: FieldAction,
    pub(crate) _inner: Field,
}

impl ToTokens for VersionedField {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
    }
}

impl VersionedField {
    pub(crate) fn new(field: Field, action: FieldAction) -> Self {
        Self {
            _inner: field,
            _action: action,
        }
    }
}
