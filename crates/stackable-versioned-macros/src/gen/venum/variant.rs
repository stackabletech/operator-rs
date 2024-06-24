use syn::Variant;

use crate::{
    attrs::variant::VariantAttributes,
    gen::common::{Item, VersionChain},
};

#[derive(Debug)]
pub(crate) struct VersionedVariant {
    chain: Option<VersionChain>,
    inner: Variant,
}

impl Item<Variant, VariantAttributes> for VersionedVariant {
    fn new(variant: Variant, attributes: VariantAttributes) -> Self {
        todo!()
    }

    fn insert_container_versions(&mut self, versions: &[crate::gen::common::ContainerVersion]) {
        todo!()
    }

    fn generate_for_container(
        &self,
        container_version: &crate::gen::common::ContainerVersion,
    ) -> Option<proc_macro2::TokenStream> {
        todo!()
    }

    fn generate_for_from_impl(
        &self,
        version: &crate::gen::common::ContainerVersion,
        next_version: &crate::gen::common::ContainerVersion,
        from_ident: &syn::Ident,
    ) -> proc_macro2::TokenStream {
        todo!()
    }

    fn get_ident(&self, version: &crate::gen::common::ContainerVersion) -> Option<&syn::Ident> {
        todo!()
    }
}
