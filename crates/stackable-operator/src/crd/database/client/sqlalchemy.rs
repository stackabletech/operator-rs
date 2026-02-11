use k8s_openapi::api::core::v1::EnvVar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::builder::pod::{container::ContainerBuilder, env::env_var_from_secret};

/// TODO docs
pub trait SQLAlchemyDatabaseConnection {
    /// TODO docs, e.g. on what are valid characters for unique_database_name
    fn sqlalchemy_connection_details(
        &self,
        unique_database_name: &str,
    ) -> SQLAlchemyDatabaseConnectionDetails;
}

pub struct SQLAlchemyDatabaseConnectionDetails {
    /// The connection URI, which can contain env variable templates, e.g.
    /// `postgresql+psycopg2://${METADATA_DATABASE_USERNAME}:${METADATA_DATABASE_PASSWORD}@airflow-postgresql:5432/airflow`
    /// or
    /// `<generic URI from the user>`.
    pub uri_template: String,

    /// The [`EnvVar`]s the operator needs to mount into the created Pods.
    pub env_vars: Vec<EnvVar>,
}

impl SQLAlchemyDatabaseConnectionDetails {
    pub fn add_to_container(&self, cb: &mut ContainerBuilder) {
        cb.add_env_vars(self.env_vars.iter());
    }
}

/// TODO docs
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericSQLAlchemyDatabaseConnection {
    /// The name of the Secret that contains an `uri` key with the complete SQLAlchemy URI.
    pub uri_secret: String,
}

impl SQLAlchemyDatabaseConnection for GenericSQLAlchemyDatabaseConnection {
    fn sqlalchemy_connection_details(
        &self,
        unique_database_name: &str,
    ) -> SQLAlchemyDatabaseConnectionDetails {
        let uri_env_name = format!(
            "{upper}_DATABASE_URI",
            upper = unique_database_name.to_uppercase()
        );
        let uri_env_var = env_var_from_secret(&uri_env_name, &self.uri_secret, "uri");

        SQLAlchemyDatabaseConnectionDetails {
            uri_template: format!("${{{uri_env_name}}}"),
            env_vars: vec![uri_env_var],
        }
    }
}
