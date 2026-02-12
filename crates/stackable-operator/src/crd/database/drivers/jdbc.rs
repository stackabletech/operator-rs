use k8s_openapi::api::core::v1::EnvVar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    builder::pod::container::ContainerBuilder, crd::database::helpers::username_and_password_envs,
};

/// TODO docs
pub trait JDBCDatabaseConnection {
    /// TODO docs
    fn jdbc_connection_details(
        &self,
        unique_database_name: &str,
    ) -> Result<JDBCDatabaseConnectionDetails, crate::crd::database::Error>;
}

pub struct JDBCDatabaseConnectionDetails {
    /// The Java class name of the driver, e.g. `org.postgresql.Driver`
    pub driver: String,

    /// The connection URI (without user and  password), e.g.
    /// `jdbc:postgresql://airflow-postgresql:5432/airflow`
    pub connection_uri: Url,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the username.
    pub username_env: Option<EnvVar>,

    /// The [`EnvVar`] that mounts the credentials Secret and provides the password.
    pub password_env: Option<EnvVar>,
}

impl JDBCDatabaseConnectionDetails {
    pub fn add_to_container(&self, cb: &mut ContainerBuilder) {
        let env_vars = self.username_env.iter().chain(self.password_env.iter());
        cb.add_env_vars(env_vars);
    }
}

/// TODO docs
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericJDBCDatabaseConnection {
    /// TODO docs
    pub driver: String,

    /// TODO docs
    pub uri: Url,

    /// TODO docs
    pub credentials_secret: String,
}

impl JDBCDatabaseConnection for GenericJDBCDatabaseConnection {
    fn jdbc_connection_details(
        &self,
        unique_database_name: &str,
    ) -> Result<JDBCDatabaseConnectionDetails, crate::crd::database::Error> {
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, &self.credentials_secret);

        Ok(JDBCDatabaseConnectionDetails {
            driver: self.driver.clone(),
            connection_uri: self.uri.clone(),
            username_env: Some(username_env),
            password_env: Some(password_env),
        })
    }
}
