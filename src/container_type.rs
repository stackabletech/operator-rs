use k8s_openapi::{
    api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity,
};

/// Describes the type of container. The types are:
///
/// - `Main`: This type of container runs the main appliction, like Superset or
///    NiFi
/// - `Init`: This type of container runs only once at the start of the product
///    cluster and executes initial tasks like database migrations or insertion
///    of data
/// - `Sidecar`: Sidecar containers run alongside the main containers for
///    additional tasks
#[derive(Clone, Debug, Default)]
pub enum ContainerType {
    #[default]
    Main,
    Init,
    Sidecar,
}

impl From<ContainerType> for ResourceRequirements {
    fn from(container_type: ContainerType) -> Self {
        Self::from(&container_type)
    }
}

impl From<&ContainerType> for ResourceRequirements {
    fn from(container_type: &ContainerType) -> Self {
        match container_type {
            ContainerType::Main => ResourceRequirements {
                limits: Some(
                    [
                        ("cpu".into(), Quantity("4".into())),
                        ("memory".into(), Quantity("4Gi".into())),
                    ]
                    .into(),
                ),
                requests: Some(
                    [
                        ("cpu".into(), Quantity("2".into())),
                        ("memory".into(), Quantity("2Gi".into())),
                    ]
                    .into(),
                ),
                ..Default::default()
            },
            ContainerType::Init => ResourceRequirements {
                limits: Some(
                    [
                        ("cpu".into(), Quantity("10m".into())),
                        ("memory".into(), Quantity("128Mi".into())),
                    ]
                    .into(),
                ),
                requests: Some(
                    [
                        ("cpu".into(), Quantity("10m".into())),
                        ("memory".into(), Quantity("128Mi".into())),
                    ]
                    .into(),
                ),
                ..Default::default()
            },
            ContainerType::Sidecar => ResourceRequirements {
                limits: Some(
                    [
                        ("cpu".into(), Quantity("2".into())),
                        ("memory".into(), Quantity("2Gi".into())),
                    ]
                    .into(),
                ),
                requests: Some(
                    [
                        ("cpu".into(), Quantity("1".into())),
                        ("memory".into(), Quantity("1Gi".into())),
                    ]
                    .into(),
                ),
                ..Default::default()
            },
        }
    }
}
