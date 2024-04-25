use k8s_version::Version;

#[derive(Debug)]
pub(crate) struct ContainerVersion {
    pub(crate) _deprecated: bool,
    pub(crate) inner: Version,
}
