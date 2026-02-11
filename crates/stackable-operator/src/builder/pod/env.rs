use k8s_openapi::api::core::v1::{EnvVar, EnvVarSource, SecretKeySelector};

pub fn env_var_from_secret(
    env_var_name: impl Into<String>,
    secret_name: impl Into<String>,
    secret_key: impl Into<String>,
) -> EnvVar {
    EnvVar {
        name: env_var_name.into(),
        value_from: Some(EnvVarSource {
            secret_key_ref: Some(SecretKeySelector {
                name: secret_name.into(),
                key: secret_key.into(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
