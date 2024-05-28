use k8s_version::Version;
use syn::Ident;

#[derive(Debug, Clone)]
pub(crate) struct ContainerVersion {
    pub(crate) deprecated: bool,
    pub(crate) inner: Version,
    pub(crate) ident: Ident,
}
