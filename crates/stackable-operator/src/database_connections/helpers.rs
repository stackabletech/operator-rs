use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::EnvVar;

use crate::builder::pod::env::env_var_with_value_from_secret;

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
        env_var_with_value_from_secret(username_env_name, credentials_secret_name, "username"),
        env_var_with_value_from_secret(password_env_name, credentials_secret_name, "password"),
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

/// Returns [`None`] if no connection parameters are defined, `?key1=value1&key2=value2` otherwise.
//
// TODO: Do we need to escape anything here? Ideally the products themselves take care of this.
// Additionally, we need to keep in mind that whatever escaping we come up with needs to be
// understood by all products (which includes JDBC, SQLAlchemy and Celery connections strings).
pub fn connection_parameters_as_url_query_parameters(
    parameters: &BTreeMap<String, String>,
) -> Option<String> {
    if parameters.is_empty() {
        return None;
    }

    let parameters = parameters
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    Some(format!("?{parameters}"))
}
