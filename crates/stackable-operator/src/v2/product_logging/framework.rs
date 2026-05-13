use std::{fmt::Display, str::FromStr};

use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::{
    builder::pod::{container::FieldPathEnvVar, resources::ResourceRequirementsBuilder},
    commons::product_image_selection::ResolvedProductImage,
    k8s_openapi::api::core::v1::{Container, VolumeMount},
    product_logging::{
        framework::VECTOR_CONFIG_FILE,
        spec::{
            AppenderConfig, AutomaticContainerLogConfig, ConfigMapLogConfig,
            ContainerLogConfigChoice, CustomContainerLogConfig, LogLevel, Logging,
        },
    },
};
use strum::{EnumDiscriminants, IntoStaticStr};

use crate::{
    constant,
    framework::{
        builder::pod::container::{EnvVarName, EnvVarSet, new_container_builder},
        role_group_utils,
        types::kubernetes::{ConfigMapKey, ConfigMapName, ContainerName, VolumeName},
    },
};

// Copy of the private constant `stackable_operator::product_logging::framework::STACKABLE_CONFIG_DIR`
const STACKABLE_CONFIG_DIR: &str = "/stackable/config";

// Copy of the private constant `stackable_operator::product_logging::framework::VECTOR_LOG_DIR`
const VECTOR_CONTROL_DIR: &str = "_vector";

// Copy of the private constant `stackable_operator::product_logging::framework::VECTOR_STATE_DIR`
const VECTOR_STATE_DIR: &str = "_vector-state";

// Copy of the private constant `stackable_operator::product_logging::framework::SHUTDOWN_FILE`
const SHUTDOWN_FILE: &str = "shutdown";

// Public variant of `stackable_operator::product_logging::framework::STACKABLE_LOG_DIR`
/// Directory where the logs are stored
pub const STACKABLE_LOG_DIR: &str = "/stackable/log";

// Copy of the private constant `stackable_operator::product_logging::framework::VECTOR_AGGREGATOR_CM_KEY`
constant!(VECTOR_AGGREGATOR_CM_KEY: ConfigMapKey = "ADDRESS");

// Copy of the private constant `stackable_operator::product_logging::framework::VECTOR_AGGREGATOR_ADDRESS`
constant!(VECTOR_AGGREGATOR_ENV_NAME: EnvVarName = "VECTOR_AGGREGATOR_ADDRESS");

#[derive(Debug, EnumDiscriminants, Snafu)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display("failed to get the container log configuration"))]
    GetContainerLogConfiguration { container: String },

    #[snafu(display("failed to parse the container name"))]
    ParseContainerName {
        source: crate::framework::macros::attributed_string_type::Error,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// Validated [`ContainerLogConfigChoice`]
///
/// The ConfigMap name in the Custom variant is valid.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidatedContainerLogConfigChoice {
    Automatic(AutomaticContainerLogConfig),
    Custom(ConfigMapName),
}

/// Validated [`ContainerLogConfigChoice`] for the Vector container
///
/// It includes the discovery ConfigMap name of the Vector aggregator.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorContainerLogConfig {
    pub log_config: ValidatedContainerLogConfigChoice,
    pub vector_aggregator_config_map_name: ConfigMapName,
}

/// Validates the log configuration of the container
pub fn validate_logging_configuration_for_container<T>(
    logging: &Logging<T>,
    container: T,
) -> Result<ValidatedContainerLogConfigChoice>
where
    T: Clone + Display + Ord,
{
    let container_log_config_choice = logging
        .containers
        .get(&container)
        .and_then(|container_log_config| container_log_config.choice.as_ref())
        // This should never happen because default configurations should have been set for all
        // containers.
        .context(GetContainerLogConfigurationSnafu {
            container: container.to_string(),
        })?;

    let validated_container_log_config_choice = match container_log_config_choice {
        ContainerLogConfigChoice::Automatic(automatic_log_config) => {
            ValidatedContainerLogConfigChoice::Automatic(automatic_log_config.clone())
        }
        ContainerLogConfigChoice::Custom(CustomContainerLogConfig {
            custom: ConfigMapLogConfig { config_map },
        }) => ValidatedContainerLogConfigChoice::Custom(
            ConfigMapName::from_str(config_map).context(ParseContainerNameSnafu)?,
        ),
    };

    Ok(validated_container_log_config_choice)
}

/// Builds the Vector container
pub fn vector_container(
    container_name: &ContainerName,
    image: &ResolvedProductImage,
    vector_container_log_config: &VectorContainerLogConfig,
    resource_names: &role_group_utils::ResourceNames,
    log_config_volume_name: &VolumeName,
    log_volume_name: &VolumeName,
    extra_env_vars: EnvVarSet,
) -> Container {
    let log_level = if let ValidatedContainerLogConfigChoice::Automatic(log_config) =
        &vector_container_log_config.log_config
    {
        log_config.root_log_level()
    } else {
        LogLevel::default()
    };
    let vector_file_log_level =
        if let ValidatedContainerLogConfigChoice::Automatic(AutomaticContainerLogConfig {
            file: Some(AppenderConfig {
                level: Some(log_level),
            }),
            ..
        }) = vector_container_log_config.log_config
        {
            log_level
        } else {
            LogLevel::default()
        };

    let env_vars = EnvVarSet::new()
        .with_value(
            &EnvVarName::from_str_unsafe("CLUSTER_NAME"),
            &resource_names.cluster_name,
        )
        .with_value(
            &EnvVarName::from_str_unsafe("DATA_DIR"),
            format!("{STACKABLE_LOG_DIR}/{VECTOR_STATE_DIR}"),
        )
        .with_value(&EnvVarName::from_str_unsafe("LOG_DIR"), STACKABLE_LOG_DIR)
        .with_field_path(
            &EnvVarName::from_str_unsafe("NAMESPACE"),
            FieldPathEnvVar::Namespace,
        )
        .with_value(
            &EnvVarName::from_str_unsafe("ROLE_GROUP_NAME"),
            &resource_names.role_group_name,
        )
        .with_value(
            &EnvVarName::from_str_unsafe("ROLE_NAME"),
            &resource_names.role_name,
        )
        .with_config_map_key_ref(
            &VECTOR_AGGREGATOR_ENV_NAME,
            &vector_container_log_config.vector_aggregator_config_map_name,
            &VECTOR_AGGREGATOR_CM_KEY,
        )
        .with_value(
            &EnvVarName::from_str_unsafe("VECTOR_CONFIG_YAML"),
            format!("{STACKABLE_CONFIG_DIR}/{VECTOR_CONFIG_FILE}"),
        )
        .with_value(
            &EnvVarName::from_str_unsafe("VECTOR_FILE_LOG_LEVEL"),
            vector_file_log_level.to_vector_literal(),
        )
        .with_value(
            &EnvVarName::from_str_unsafe("VECTOR_LOG"),
            log_level.to_vector_literal(),
        )
        .merge(extra_env_vars);

    let resources = ResourceRequirementsBuilder::new()
        .with_cpu_request("250m")
        .with_cpu_limit("500m")
        .with_memory_request("128Mi")
        .with_memory_limit("128Mi")
        .build();

    new_container_builder(container_name)
            .image_from_product_image(image)
            .command(vec![
                "/bin/bash".to_string(),
                "-x".to_string(),
                "-euo".to_string(),
                "pipefail".to_string(),
                "-c".to_string(),
            ])
            .args(vec![format!(
                "mkdir --parents {STACKABLE_LOG_DIR}/{VECTOR_STATE_DIR}\n\
                # Vector will ignore SIGTERM (as PID != 1) and must be shut down by writing a shutdown trigger file\n\
                vector & vector_pid=$!\n\
                if [ ! -f \"{vector_control_directory}/{SHUTDOWN_FILE}\" ]; then\n\
                    mkdir -p {vector_control_directory}\n\
                    inotifywait -qq --event create {vector_control_directory};\n\
                fi\n\
                sleep 1\n\
                kill $vector_pid",
                vector_control_directory = format!("{STACKABLE_LOG_DIR}/{VECTOR_CONTROL_DIR}"),
            )])
            .add_env_vars(env_vars)
            .add_volume_mounts([
                VolumeMount {
                    mount_path: format!(
                        "{STACKABLE_CONFIG_DIR}/{VECTOR_CONFIG_FILE}"
                    ),
                    name: log_config_volume_name.to_string(),
                    read_only: Some(true),
                    sub_path: Some(VECTOR_CONFIG_FILE.to_owned()),
                    ..VolumeMount::default()
                },
                VolumeMount {
                    mount_path: STACKABLE_LOG_DIR.to_owned(),
                    name: log_volume_name.to_string(),
                    ..VolumeMount::default()
                },
            ])
            .expect("The mount paths are statically defined and there should be no duplicates.")
            .resources(resources)
            .build()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use pretty_assertions::assert_eq;
    use serde_json::json;
    use stackable_operator::{
        commons::product_image_selection::ResolvedProductImage,
        kvp::LabelValue,
        product_logging::spec::{
            AutomaticContainerLogConfig, ConfigMapLogConfig, ContainerLogConfig,
            ContainerLogConfigChoice, CustomContainerLogConfig, Logging,
        },
    };

    use super::{
        ErrorDiscriminants, ValidatedContainerLogConfigChoice, VectorContainerLogConfig,
        validate_logging_configuration_for_container, vector_container,
    };
    use crate::framework::{
        builder::pod::container::{EnvVarName, EnvVarSet},
        role_group_utils,
        types::{
            kubernetes::{ConfigMapName, ContainerName, VolumeName},
            operator::{ClusterName, RoleGroupName, RoleName},
        },
    };

    #[test]
    fn test_validate_logging_configuration_for_container_ok_automatic_log_config() {
        let logging = Logging {
            enable_vector_agent: false,
            containers: [(
                "container",
                ContainerLogConfig {
                    choice: Some(ContainerLogConfigChoice::Automatic(
                        AutomaticContainerLogConfig::default(),
                    )),
                },
            )]
            .into(),
        };

        assert_eq!(
            ValidatedContainerLogConfigChoice::Automatic(AutomaticContainerLogConfig::default()),
            validate_logging_configuration_for_container(&logging, "container")
                .expect("should be a valid log config")
        );
    }

    #[test]
    fn test_validate_logging_configuration_for_container_ok_custom_log_config() {
        let logging = Logging {
            enable_vector_agent: false,
            containers: [(
                "container",
                ContainerLogConfig {
                    choice: Some(ContainerLogConfigChoice::Custom(CustomContainerLogConfig {
                        custom: ConfigMapLogConfig {
                            config_map: "valid-config-map-name".to_owned(),
                        },
                    })),
                },
            )]
            .into(),
        };

        assert_eq!(
            ValidatedContainerLogConfigChoice::Custom(ConfigMapName::from_str_unsafe(
                "valid-config-map-name"
            )),
            validate_logging_configuration_for_container(&logging, "container")
                .expect("should be a valid log config")
        );
    }

    #[test]
    fn test_validate_logging_configuration_for_container_err_get_container_log_configuration() {
        let logging_without_container = Logging {
            enable_vector_agent: false,
            containers: [].into(),
        };
        let logging_without_container_log_config_choice = Logging {
            enable_vector_agent: false,
            containers: [("container", ContainerLogConfig { choice: None })].into(),
        };

        assert_eq!(
            Err(ErrorDiscriminants::GetContainerLogConfiguration),
            validate_logging_configuration_for_container(&logging_without_container, "container")
                .map_err(ErrorDiscriminants::from)
        );

        assert_eq!(
            Err(ErrorDiscriminants::GetContainerLogConfiguration),
            validate_logging_configuration_for_container(
                &logging_without_container_log_config_choice,
                "container"
            )
            .map_err(ErrorDiscriminants::from)
        );
    }

    #[test]
    fn test_validate_logging_configuration_for_container_err_parse_container_name() {
        let logging = Logging {
            enable_vector_agent: false,
            containers: [(
                "container",
                ContainerLogConfig {
                    choice: Some(ContainerLogConfigChoice::Custom(CustomContainerLogConfig {
                        custom: ConfigMapLogConfig {
                            config_map: "invalid ConfigMap name".to_owned(),
                        },
                    })),
                },
            )]
            .into(),
        };

        assert_eq!(
            Err(ErrorDiscriminants::ParseContainerName),
            validate_logging_configuration_for_container(&logging, "container")
                .map_err(ErrorDiscriminants::from)
        );
    }

    #[test]
    fn test_vector_container() {
        let image = ResolvedProductImage {
            product_version: "1.0.0".to_owned(),
            app_version_label_value: LabelValue::from_str("1.0.0-stackable0.0.0-dev")
                .expect("should be a valid label value"),
            image: "oci.stackable.tech/sdp/product:1.0.0-stackable0.0.0-dev".to_string(),
            image_pull_policy: "Always".to_owned(),
            pull_secrets: None,
        };

        let vector_container_log_config = VectorContainerLogConfig {
            log_config: ValidatedContainerLogConfigChoice::Automatic(
                AutomaticContainerLogConfig::default(),
            ),
            vector_aggregator_config_map_name: ConfigMapName::from_str_unsafe("vector-aggregator"),
        };

        let resource_names = role_group_utils::ResourceNames {
            cluster_name: ClusterName::from_str_unsafe("test-cluster"),
            role_name: RoleName::from_str_unsafe("role"),
            role_group_name: RoleGroupName::from_str_unsafe("role-group"),
        };

        let vector_container = vector_container(
            &ContainerName::from_str_unsafe("vector"),
            &image,
            &vector_container_log_config,
            &resource_names,
            &VolumeName::from_str_unsafe("config"),
            &VolumeName::from_str_unsafe("log"),
            EnvVarSet::new().with_value(&EnvVarName::from_str_unsafe("CUSTOM_ENV"), "test"),
        );

        assert_eq!(
            json!(
            {
                "args": [
                    concat!(
                        "mkdir --parents /stackable/log/_vector-state\n",
                        "# Vector will ignore SIGTERM (as PID != 1) and must be shut down by writing a shutdown trigger file\n",
                        "vector & vector_pid=$!\n",
                        "if [ ! -f \"/stackable/log/_vector/shutdown\" ]; then\n",
                        "mkdir -p /stackable/log/_vector\n",
                        "inotifywait -qq --event create /stackable/log/_vector;\n",
                        "fi\n",
                        "sleep 1\n",
                        "kill $vector_pid"
                    ),
                ],
                "command": [
                    "/bin/bash",
                    "-x",
                    "-euo",
                    "pipefail",
                    "-c",
                ],
                "env": [
                    {
                        "name": "CLUSTER_NAME",
                        "value": "test-cluster",
                    },
                    {
                        "name": "CUSTOM_ENV",
                        "value": "test",
                    },
                    {
                        "name": "DATA_DIR",
                        "value": "/stackable/log/_vector-state",
                    },
                    {
                        "name": "LOG_DIR",
                        "value": "/stackable/log",
                    },
                    {
                        "name": "NAMESPACE",
                        "valueFrom": {
                            "fieldRef": {
                                "fieldPath": "metadata.namespace",
                            },
                        },
                    },
                    {
                        "name": "ROLE_GROUP_NAME",
                        "value": "role-group",
                    },
                    {
                        "name": "ROLE_NAME",
                        "value": "role",
                    },
                    {
                        "name": "VECTOR_AGGREGATOR_ADDRESS",
                        "valueFrom": {
                            "configMapKeyRef": {
                                "key": "ADDRESS",
                                "name": "vector-aggregator",
                            },
                        },
                    },
                    {
                        "name": "VECTOR_CONFIG_YAML",
                        "value": "/stackable/config/vector.yaml",
                    },
                    {
                        "name": "VECTOR_FILE_LOG_LEVEL",
                        "value": "info",
                    },
                    {
                        "name": "VECTOR_LOG",
                        "value": "info",
                    },
                ],
                "image": "oci.stackable.tech/sdp/product:1.0.0-stackable0.0.0-dev",
                "imagePullPolicy": "Always",
                "name": "vector",
                "resources": {
                    "limits": {
                        "cpu": "500m",
                        "memory": "128Mi",
                    },
                    "requests": {
                        "cpu": "250m",
                        "memory": "128Mi",
                    },
                },
                "volumeMounts": [
                    {
                        "mountPath": "/stackable/config/vector.yaml",
                        "name": "config",
                        "readOnly": true,
                        "subPath": "vector.yaml",
                    },
                    {
                        "mountPath": "/stackable/log",
                        "name": "log",
                    },
                ],
            }),
            serde_json::to_value(vector_container).expect("should be serializable")
        );
    }
}
