use k8s_openapi::api::core::v1::EnvVar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    builder::pod::{container::ContainerBuilder, env::env_var_from_secret},
    databases::TemplatingMechanism,
};

/// Implemented by database connection types that support
/// [SQLAlchemy](https://www.sqlalchemy.org/) connection URLs.
///
/// Provides a standardized way to obtain a SQLAlchemy connection URI template together with the
/// necessary credential env vars, regardless of the concrete database type.
pub trait SQLAlchemyDatabaseConnection {
    /// Returns the SQLAlchemy connection details for the given `unique_database_name` using the
    /// default [`TemplatingMechanism`].
    ///
    /// `unique_database_name` identifies this particular database connection within the operator
    /// and is used as a prefix when naming the injected environment variables. It must consist only
    /// of uppercase ASCII letters and underscores.
    fn sqlalchemy_connection_details(
        &self,
        unique_database_name: &str,
    ) -> SQLAlchemyDatabaseConnectionDetails {
        self.sqlalchemy_connection_details_with_templating(
            unique_database_name,
            &TemplatingMechanism::default(),
        )
    }

    /// Like [`Self::sqlalchemy_connection_details`], but allows specifying a [`TemplatingMechanism`]
    /// explicitly. Use this when the calling context controls how configuration files are rendered,
    /// e.g. when using bash env substitution instead of config-utils.
    fn sqlalchemy_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> SQLAlchemyDatabaseConnectionDetails;
}

pub struct SQLAlchemyDatabaseConnectionDetails {
    /// The connection URI, which can contain env variable templates, e.g.
    /// `postgresql+psycopg2://${env:METADATA_DATABASE_USERNAME}:${env:METADATA_DATABASE_PASSWORD}@airflow-postgresql:5432/airflow`
    /// or
    /// `<generic URI from the user>`.
    pub uri_template: String,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the username.
    pub username_env: Option<EnvVar>,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the password.
    pub password_env: Option<EnvVar>,

    /// The [`EnvVar`] that mounts the user-specified Secret and provides the generic URI.
    pub generic_uri_var: Option<EnvVar>,
}

impl SQLAlchemyDatabaseConnectionDetails {
    pub fn env_vars(&self) -> impl Iterator<Item = &EnvVar> {
        [
            &self.username_env,
            &self.password_env,
            &self.generic_uri_var,
        ]
        .into_iter()
        .flatten()
    }

    pub fn add_to_container(&self, cb: &mut ContainerBuilder) {
        cb.add_env_vars(self.env_vars());
    }
}

/// A generic SQLAlchemy database connection for database types not covered by a dedicated variant.
///
/// Use this when you need to connect to a SQLAlchemy-compatible database that does not have a
/// first-class connection type. The complete connection URI is read from a Secret, giving the user
/// full control over the connection string including any driver-specific options.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericSQLAlchemyDatabaseConnection {
    /// The name of the Secret that contains an `uri` key with the complete SQLAlchemy URI.
    pub uri_secret: String,
}

impl SQLAlchemyDatabaseConnection for GenericSQLAlchemyDatabaseConnection {
    fn sqlalchemy_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> SQLAlchemyDatabaseConnectionDetails {
        let uri_env_name = format!(
            "{upper}_DATABASE_URI",
            upper = unique_database_name.to_uppercase()
        );
        let uri_env_var = env_var_from_secret(&uri_env_name, &self.uri_secret, "uri");
        let uri_template = match templating_mechanism {
            TemplatingMechanism::ConfigUtils => format!("${{env:{uri_env_name}}}"),
            TemplatingMechanism::BashEnvSubstitution => format!("${{{uri_env_name}}}"),
        };

        SQLAlchemyDatabaseConnectionDetails {
            uri_template,
            username_env: None,
            password_env: None,
            generic_uri_var: Some(uri_env_var),
        }
    }
}
