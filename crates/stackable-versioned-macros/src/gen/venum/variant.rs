use syn::Ident;

use crate::gen::common::ContainerVersion;

#[derive(Debug)]
pub(crate) struct VersionedVariant {}

impl VersionedVariant {
    pub(crate) fn new() -> Self {
        todo!()
    }

    pub(crate) fn insert_container_versions(&mut self, versions: &[ContainerVersion]) {
        todo!()
    }

    pub(crate) fn get_ident(&self, version: &ContainerVersion) -> Option<&Ident> {
        // match &self.chain {
        //     Some(chain) => chain
        //         .get(&version.inner)
        //         .expect("internal error: chain must contain container version")
        //         .get_ident(),
        //     None => self.inner.ident.as_ref(),
        // }
        todo!()
    }
}
