use k8s_openapi::api::core::v1::EnvVar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    builder::pod::{container::ContainerBuilder, env::env_var_with_value_from_secret},
    database_connections::TemplatingMechanism,
};

/// Implemented by database connection types that can serve as a
/// [Celery](https://docs.celeryq.dev/) broker or result backend.
///
/// Provides a standardized way to obtain a Celery connection URL template together with the
/// necessary credential env vars, regardless of the concrete database or message broker type.
pub trait CeleryDatabaseConnection {
    /// Returns the Celery connection details for the given `unique_database_name` using the
    /// default [`TemplatingMechanism`].
    ///
    /// The `unique_database_name` identifies this particular database connection within the operator
    /// and is used as a prefix when naming the injected environment variables. It must consist only
    /// of uppercase ASCII letters and underscores.
    fn celery_connection_details(
        &self,
        unique_database_name: &str,
    ) -> CeleryDatabaseConnectionDetails {
        self.celery_connection_details_with_templating(
            unique_database_name,
            &TemplatingMechanism::default(),
        )
    }

    /// Like [`Self::celery_connection_details`], but allows specifying a [`TemplatingMechanism`]
    /// explicitly. Use this when the calling context controls how configuration files are rendered,
    /// e.g. when using bash env substitution instead of config-utils.
    fn celery_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> CeleryDatabaseConnectionDetails;
}

pub struct CeleryDatabaseConnectionDetails {
    /// The connection URL, which can contain env variable templates, e.g.
    /// `redis://:${env:METADATA_DATABASE_PASSWORD}@airflow-redis-master:6379/0`
    /// or
    /// `<generic URL from the user>`.
    pub url_template: String,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the username.
    pub username_env: Option<EnvVar>,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the password.
    pub password_env: Option<EnvVar>,

    /// The [`EnvVar`] that mounts the user-specified Secret and provides the generic URL.
    pub generic_url_var: Option<EnvVar>,
}

impl CeleryDatabaseConnectionDetails {
    pub fn env_vars(&self) -> impl Iterator<Item = &EnvVar> {
        [
            &self.username_env,
            &self.password_env,
            &self.generic_url_var,
        ]
        .into_iter()
        .flatten()
    }

    /// Adds all the needed elements (e.g. env vars or volume mounts) to the given
    /// [`ContainerBuilder`].
    ///
    /// Currently, only environment variables are added. In the future it e.g. might also add TLS
    /// ca certificate mounts.
    pub fn add_to_container(&self, cb: &mut ContainerBuilder) {
        cb.add_env_vars(self.env_vars().cloned());
    }
}

/// A generic Celery database connection for broker or result backend types not covered by a
/// dedicated variant.
///
/// Use this when you need a Celery-compatible connection that does not have a first-class
/// connection type. The complete connection URL is read from a Secret, giving the user full
/// control over the connection string.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericCeleryDatabaseConnection {
    /// The name of the Secret that contains an `connectionUrl` key with the complete Celery URL.
    pub connection_url_secret_name: String,
}

impl CeleryDatabaseConnection for GenericCeleryDatabaseConnection {
    fn celery_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> CeleryDatabaseConnectionDetails {
        let url_env_name = format!(
            "{upper}_DATABASE_URL",
            upper = unique_database_name.to_uppercase()
        );
        let url_env_var = env_var_with_value_from_secret(
            &url_env_name,
            &self.connection_url_secret_name,
            "connectionUrl",
        );
        let url_template = match templating_mechanism {
            TemplatingMechanism::ConfigUtils => format!("${{env:{url_env_name}}}"),
            TemplatingMechanism::BashEnvSubstitution => format!("${{{url_env_name}}}"),
        };

        CeleryDatabaseConnectionDetails {
            url_template,
            username_env: None,
            password_env: None,
            generic_url_var: Some(url_env_var),
        }
    }
}
