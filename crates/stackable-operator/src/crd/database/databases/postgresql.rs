use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::{
    commons::networking::HostName,
    crd::database::{
        drivers::{
            jdbc::{JDBCDatabaseConnection, JDBCDatabaseConnectionDetails},
            sqlalchemy::{SQLAlchemyDatabaseConnection, SQLAlchemyDatabaseConnectionDetails},
        },
        helpers::{connection_parameters_as_url_query_parameters, username_and_password_envs},
    },
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse connection URL"))]
    ParseConnectionUrl { source: url::ParseError },
}

/// TODO docs
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostgresqlConnection {
    /// TODO docs
    pub host: HostName,

    /// TODO docs
    #[serde(default = "PostgresqlConnection::default_port")]
    pub port: u16,

    /// TODO docs
    pub database: String,

    /// TODO docs
    pub credentials_secret: String,

    /// TODO docs
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

impl PostgresqlConnection {
    fn default_port() -> u16 {
        5432
    }
}

impl JDBCDatabaseConnection for PostgresqlConnection {
    fn jdbc_connection_details(
        &self,
        unique_database_name: &str,
    ) -> Result<JDBCDatabaseConnectionDetails, crate::crd::database::Error> {
        let Self {
            host,
            port,
            database,
            credentials_secret,
            parameters,
        } = self;
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, credentials_secret);

        let connection_uri = format!(
            "jdbc:postgresql://{host}:{port}/{database}{parameters}",
            parameters = connection_parameters_as_url_query_parameters(parameters)
        );
        let connection_uri = connection_uri.parse().context(ParseConnectionUrlSnafu)?;

        Ok(JDBCDatabaseConnectionDetails {
            driver: "org.postgresql.Driver".to_owned(),
            connection_uri,
            username_env: Some(username_env),
            password_env: Some(password_env),
        })
    }
}

impl SQLAlchemyDatabaseConnection for PostgresqlConnection {
    fn sqlalchemy_connection_details(
        &self,
        unique_database_name: &str,
    ) -> SQLAlchemyDatabaseConnectionDetails {
        let Self {
            host,
            port,
            database,
            credentials_secret,
            parameters,
        } = self;
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, credentials_secret);

        let uri_template = format!(
            "postgresql+psycopg2://${{env:{username_env_name}}}:${{env:{password_env_name}}}@{host}:{port}/{database}{parameters}",
            username_env_name = username_env.name,
            password_env_name = password_env.name,
            parameters = connection_parameters_as_url_query_parameters(parameters)
        );
        SQLAlchemyDatabaseConnectionDetails {
            uri_template,
            username_env: Some(username_env),
            password_env: Some(password_env),
            generic_uri_var: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIQUE_DATABASE_NAME: &str = "METADATA";

    #[test]
    fn test_minimal_example() {
        let postgres_connection: PostgresqlConnection = serde_yaml::from_str(
            "
            host: airflow-postgresql
            database: airflow
            credentialsSecret: airflow-postgresql-credentials
            ",
        )
        .expect("invalid test input");
        let sqlalchemy_connection_details =
            postgres_connection.sqlalchemy_connection_details(UNIQUE_DATABASE_NAME);
        assert_eq!(
            sqlalchemy_connection_details.uri_template,
            "postgresql+psycopg2://${env:METADATA_DATABASE_USERNAME}:${env:METADATA_DATABASE_PASSWORD}@airflow-postgresql:5432/airflow"
        );
        assert!(sqlalchemy_connection_details.username_env.is_some());
        assert!(sqlalchemy_connection_details.password_env.is_some());
        assert!(sqlalchemy_connection_details.generic_uri_var.is_none());

        let jdbc_connection_details = postgres_connection
            .jdbc_connection_details(UNIQUE_DATABASE_NAME)
            .expect("failed to get JDBC connection details");
        assert_eq!(jdbc_connection_details.driver, "org.postgresql.Driver");
        assert_eq!(
            jdbc_connection_details.connection_uri.to_string(),
            "jdbc:postgresql://airflow-postgresql:5432/airflow"
        );
        assert_eq!(
            jdbc_connection_details.username_env.unwrap().name,
            "METADATA_DATABASE_USERNAME"
        );
        assert_eq!(
            jdbc_connection_details.password_env.unwrap().name,
            "METADATA_DATABASE_PASSWORD"
        );
    }

    #[test]
    fn test_parameters() {
        let postgres_connection: PostgresqlConnection = serde_yaml::from_str(
            "
            host: my-airflow.default.svc.cluster.local
            database: my_database
            port: 1234
            credentialsSecret: airflow-postgresql-credentials
            parameters:
              createDatabaseIfNotExist: true
              foo: bar
            ",
        )
        .expect("invalid test input");
        let sqlalchemy_connection_details =
            postgres_connection.sqlalchemy_connection_details(UNIQUE_DATABASE_NAME);
        assert_eq!(
            sqlalchemy_connection_details.uri_template,
            "postgresql+psycopg2://${env:METADATA_DATABASE_USERNAME}:${env:METADATA_DATABASE_PASSWORD}@my-airflow.default.svc.cluster.local:1234/my_database?createDatabaseIfNotExist=true&foo=bar"
        );

        let jdbc_connection_details = postgres_connection
            .jdbc_connection_details(UNIQUE_DATABASE_NAME)
            .expect("failed to get JDBC connection details");
        assert_eq!(
            jdbc_connection_details.connection_uri.to_string(),
            "jdbc:postgresql://my-airflow.default.svc.cluster.local:1234/my_database?createDatabaseIfNotExist=true&foo=bar"
        );
    }
}
