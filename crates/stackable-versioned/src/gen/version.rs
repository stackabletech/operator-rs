use k8s_version::Version;

pub(crate) struct ContainerVersion {
    pub(crate) _deprecated: bool,
    pub(crate) inner: Version,
}
