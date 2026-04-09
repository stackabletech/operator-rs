use k8s_openapi::api::core::v1::EnvVar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    builder::pod::container::ContainerBuilder,
    database_connections::{TemplatingMechanism, helpers::username_and_password_envs},
};

/// Implemented by database connection types that support JDBC.
///
/// Provides a standardized way to obtain JDBC connection details (driver class, URL, and
/// credential env vars) regardless of the concrete database type.
pub trait JdbcDatabaseConnection {
    /// Returns the JDBC connection details for the given `unique_database_name` using the
    /// default [`TemplatingMechanism`].
    ///
    /// The `unique_database_name` identifies this particular database connection within the operator
    /// and is used as a prefix when naming the injected environment variables. It must consist only
    /// of uppercase ASCII letters and underscores.
    fn jdbc_connection_details(
        &self,
        unique_database_name: &str,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::database_connections::Error> {
        self.jdbc_connection_details_with_templating(
            unique_database_name,
            &TemplatingMechanism::default(),
        )
    }

    /// Like [`Self::jdbc_connection_details`], but allows specifying a [`TemplatingMechanism`]
    /// explicitly. Use this when the calling context controls how configuration files are rendered,
    /// e.g. when using bash env substitution instead of config-utils.
    fn jdbc_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::database_connections::Error>;
}

pub struct JdbcDatabaseConnectionDetails {
    /// The Java class name of the driver, e.g. `org.postgresql.Driver`
    pub driver: String,

    /// The connection URL (without user and password), e.g.
    /// `jdbc:postgresql://airflow-postgresql:5432/airflow`
    pub connection_url: Url,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the username.
    pub username_env: Option<EnvVar>,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the password.
    pub password_env: Option<EnvVar>,
}

impl JdbcDatabaseConnectionDetails {
    /// Adds all the needed elements (e.g. env vars or volume mounts) to the given
    /// [`ContainerBuilder`].
    ///
    /// Currently, only (optionally) environment variables for the username and password are added.
    /// In the future it e.g. might also add TLS ca certificate mounts.
    pub fn add_to_container(&self, cb: &mut ContainerBuilder) {
        let env_vars = self.username_env.iter().chain(self.password_env.iter());
        cb.add_env_vars(env_vars.cloned());
    }
}

/// A generic JDBC database connection for database types not covered by a dedicated variant.
///
/// Use this when you need to connect to a JDBC-compatible database that does not have a
/// first-class connection type. You are responsible for providing the correct driver class name
/// and a fully-formed JDBC URL as well as providing the needed classes on the Java classpath.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericJdbcDatabaseConnection {
    /// Fully-qualified Java class name of the JDBC driver, e.g. `org.postgresql.Driver` or
    /// `com.mysql.jdbc.Driver`. The driver JAR must be provided by you on the classpath.
    pub driver: String,

    /// The JDBC connection URL, e.g. `jdbc:postgresql://my-host:5432/mydb`. Credentials must
    /// not be embedded in this URL; they are instead injected via environment variables sourced
    /// from `credentials_secret`.
    pub url: Url,

    /// Name of a Secret containing the `username` and `password` keys used to authenticate
    /// against the database.
    pub credentials_secret_name: String,
}

impl JdbcDatabaseConnection for GenericJdbcDatabaseConnection {
    fn jdbc_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        _templating_mechanism: &TemplatingMechanism,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::database_connections::Error> {
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, &self.credentials_secret_name);

        Ok(JdbcDatabaseConnectionDetails {
            driver: self.driver.clone(),
            connection_url: self.url.clone(),
            username_env: Some(username_env),
            password_env: Some(password_env),
        })
    }
}
