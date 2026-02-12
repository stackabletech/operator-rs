use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::{
    commons::networking::HostName,
    crd::database::{
        drivers::jdbc::{JDBCDatabaseConnection, JDBCDatabaseConnectionDetails},
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
pub struct MysqlConnection {
    /// TODO docs
    pub host: HostName,

    /// TODO docs
    #[serde(default = "MysqlConnection::default_port")]
    pub port: u16,

    /// TODO docs
    pub database: String,

    /// TODO docs
    pub credentials_secret: String,

    /// TODO docs
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

impl MysqlConnection {
    fn default_port() -> u16 {
        3306
    }
}

impl JDBCDatabaseConnection for MysqlConnection {
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
            "jdbc:mysql://{host}:{port}/{database}{parameters}",
            parameters = connection_parameters_as_url_query_parameters(parameters)
        );
        let connection_uri = connection_uri.parse().context(ParseConnectionUrlSnafu)?;

        Ok(JDBCDatabaseConnectionDetails {
            driver: "com.mysql.jdbc.Driver".to_owned(),
            connection_uri,
            username_env: Some(username_env),
            password_env: Some(password_env),
        })
    }
}
