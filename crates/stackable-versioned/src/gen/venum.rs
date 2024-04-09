use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{DataEnum, Ident, Result};

use crate::attrs::container::ContainerAttributes;

pub(crate) struct VersionedEnum {}

impl VersionedEnum {
    pub(crate) fn new(
        _ident: Ident,
        _data: DataEnum,
        _attributes: ContainerAttributes,
    ) -> Result<Self> {
        todo!()
    }
}

impl ToTokens for VersionedEnum {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
    }
}
