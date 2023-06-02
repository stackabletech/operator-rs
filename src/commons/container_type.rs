#[derive(Clone, Copy, Debug, Default)]
pub enum ContainerType {
    Sidecar(SidecarContainerType),
    Init(InitContainerType),

    #[default]
    Main,
}

#[derive(Clone, Copy, Debug)]
pub enum SidecarContainerType {
    Healthcheck,
    Logging,
    Metrics,
}

#[derive(Clone, Copy, Debug)]
pub enum InitContainerType {
    FilesystemActions,
    TextReplacement,
    Security,
}
