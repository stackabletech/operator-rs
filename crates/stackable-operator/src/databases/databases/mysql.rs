use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::{
    commons::networking::HostName,
    databases::{
        TemplatingMechanism,
        drivers::jdbc::{JdbcDatabaseConnection, JdbcDatabaseConnectionDetails},
        helpers::{connection_parameters_as_url_query_parameters, username_and_password_envs},
    },
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse connection URL"))]
    ParseConnectionUrl { source: url::ParseError },
}

/// Connection settings for a [MySQL](https://www.mysql.com/) database.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MysqlConnection {
    /// Hostname or IP address of the MySQL server.
    pub host: HostName,

    /// Port the MySQL server is listening on. Defaults to `3306`.
    #[serde(default = "MysqlConnection::default_port")]
    pub port: u16,

    /// Name of the database (schema) to connect to.
    pub database: String,

    /// Name of a Secret containing the `username` and `password` keys used to authenticate
    /// against the MySQL server.
    pub credentials_secret: String,

    /// Additional map of connection parameters to append to the connection URL. The given
    /// `HashMap<String, String>` will be converted to query parameters in the form of
    /// `?param1=value1&param2=value2`.
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

impl MysqlConnection {
    fn default_port() -> u16 {
        3306
    }
}

impl JdbcDatabaseConnection for MysqlConnection {
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
            "jdbc:mysql://{host}:{port}/{database}{parameters}",
            parameters =
                connection_parameters_as_url_query_parameters(parameters).unwrap_or_default()
        );
        let connection_uri = connection_uri.parse().context(ParseConnectionUrlSnafu)?;

        Ok(JdbcDatabaseConnectionDetails {
            driver: "com.mysql.jdbc.Driver".to_owned(),
            connection_uri,
            username_env: Some(username_env),
            password_env: Some(password_env),
        })
    }
}
