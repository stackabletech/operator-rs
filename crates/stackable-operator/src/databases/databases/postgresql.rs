use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::{
    commons::networking::HostName,
    databases::{
        TemplatingMechanism,
        drivers::{
            celery::{CeleryDatabaseConnection, CeleryDatabaseConnectionDetails},
            jdbc::{JdbcDatabaseConnection, JdbcDatabaseConnectionDetails},
            sqlalchemy::{SqlAlchemyDatabaseConnection, SqlAlchemyDatabaseConnectionDetails},
        },
        helpers::{connection_parameters_as_url_query_parameters, username_and_password_envs},
    },
};

pub const POSTGRES_JDBC_DRIVER_CLASS: &str = "org.postgresql.Driver";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse connection URL"))]
    ParseConnectionUrl { source: url::ParseError },
}

/// Connection settings for a [PostgreSQL](https://www.postgresql.org/) database.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostgresqlConnection {
    /// Hostname or IP address of the PostgreSQL server.
    pub host: HostName,

    /// Port the PostgreSQL server is listening on. Defaults to `5432`.
    #[serde(default = "PostgresqlConnection::default_port")]
    pub port: u16,

    /// Name of the database (schema) to connect to.
    pub database: String,

    /// Name of a Secret containing the `username` and `password` keys used to authenticate
    /// against the PostgreSQL server.
    pub credentials_secret: String,

    /// Additional map of JDBC connection parameters to append to the connection URL. The given
    /// `HashMap<String, String>` will be converted to query parameters in the form of
    /// `?param1=value1&param2=value2`.
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

impl PostgresqlConnection {
    fn default_port() -> u16 {
        5432
    }
}

impl JdbcDatabaseConnection for PostgresqlConnection {
    fn jdbc_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        _templating_mechanism: &TemplatingMechanism,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::databases::Error> {
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
            parameters =
                connection_parameters_as_url_query_parameters(parameters).unwrap_or_default()
        );
        let connection_uri = connection_uri.parse().context(ParseConnectionUrlSnafu)?;

        Ok(JdbcDatabaseConnectionDetails {
            driver: POSTGRES_JDBC_DRIVER_CLASS.to_owned(),
            connection_uri,
            username_env: Some(username_env),
            password_env: Some(password_env),
        })
    }
}

impl SqlAlchemyDatabaseConnection for PostgresqlConnection {
    fn sqlalchemy_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> SqlAlchemyDatabaseConnectionDetails {
        let Self {
            host,
            port,
            database,
            credentials_secret,
            parameters,
        } = self;
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, credentials_secret);
        let username_env_name = &username_env.name;
        let password_env_name = &password_env.name;
        let parameters =
            connection_parameters_as_url_query_parameters(parameters).unwrap_or_default();

        let uri_template = match templating_mechanism {
            TemplatingMechanism::ConfigUtils => format!(
                "postgresql+psycopg2://${{env:{username_env_name}}}:${{env:{password_env_name}}}@{host}:{port}/{database}{parameters}",
            ),
            TemplatingMechanism::BashEnvSubstitution => format!(
                "postgresql+psycopg2://${{{username_env_name}}}:${{{password_env_name}}}@{host}:{port}/{database}{parameters}",
            ),
        };
        SqlAlchemyDatabaseConnectionDetails {
            uri_template,
            username_env: Some(username_env),
            password_env: Some(password_env),
            generic_uri_var: None,
        }
    }
}

impl CeleryDatabaseConnection for PostgresqlConnection {
    fn celery_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> CeleryDatabaseConnectionDetails {
        let Self {
            host,
            port,
            database,
            credentials_secret,
            parameters,
        } = self;
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, credentials_secret);
        let username_env_name = &username_env.name;
        let password_env_name = &password_env.name;
        let parameters =
            connection_parameters_as_url_query_parameters(parameters).unwrap_or_default();

        let uri_template = match templating_mechanism {
            TemplatingMechanism::ConfigUtils => format!(
                "db+postgresql://${{env:{username_env_name}}}:${{env:{password_env_name}}}@{host}:{port}/{database}{parameters}",
            ),
            TemplatingMechanism::BashEnvSubstitution => format!(
                "db+postgresql://${{{username_env_name}}}:${{{password_env_name}}}@{host}:{port}/{database}{parameters}",
            ),
        };
        CeleryDatabaseConnectionDetails {
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
        assert_eq!(jdbc_connection_details.driver, POSTGRES_JDBC_DRIVER_CLASS);
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

        let celery_connection_details =
            postgres_connection.celery_connection_details(UNIQUE_DATABASE_NAME);
        assert_eq!(
            celery_connection_details.uri_template,
            "db+postgresql://${env:METADATA_DATABASE_USERNAME}:${env:METADATA_DATABASE_PASSWORD}@airflow-postgresql:5432/airflow"
        );
        assert!(celery_connection_details.username_env.is_some());
        assert!(celery_connection_details.password_env.is_some());
        assert!(celery_connection_details.generic_uri_var.is_none());
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
