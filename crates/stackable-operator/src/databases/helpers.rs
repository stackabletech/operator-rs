use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::EnvVar;

use crate::builder::pod::env::env_var_from_secret;

/// Returns the needed [`EnvVar`] mounts for username and password.
///
/// They will mount the specified Secret as env var into the Pod.
pub fn username_and_password_envs(
    unique_database_name: &str,
    credentials_secret_name: &str,
) -> (EnvVar, EnvVar) {
    let (username_env_name, password_env_name) =
        username_and_password_env_names(unique_database_name);
    (
        env_var_from_secret(username_env_name, credentials_secret_name, "username"),
        env_var_from_secret(password_env_name, credentials_secret_name, "password"),
    )
}

pub fn username_and_password_env_names(unique_database_name: &str) -> (String, String) {
    let env_name_prefix = format!(
        "{upper}_DATABASE",
        upper = unique_database_name.to_uppercase()
    );
    (
        format!("{env_name_prefix}_USERNAME"),
        format!("{env_name_prefix}_PASSWORD"),
    )
}

/// Returns
///
/// * If no params are defined: ""
/// * If params are defined: "?key=value&foo=bar"
pub fn connection_parameters_as_url_query_parameters(
    parameters: &BTreeMap<String, String>,
) -> String {
    if parameters.is_empty() {
        String::new()
    } else {
        let parameters = parameters
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{parameters}")
    }
}
