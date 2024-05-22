use k8s_version::Version;

#[derive(Debug, Clone)]
pub(crate) struct ContainerVersion {
    pub(crate) deprecated: bool,
    pub(crate) inner: Version,
}
