use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::crd::database::drivers::jdbc::{JDBCDatabaseConnection, JDBCDatabaseConnectionDetails};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse connection URL"))]
    ParseConnectionUrl { source: url::ParseError },
}

/// TODO docs
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerbyConnection {
    /// TODO docs, especially on default
    pub location: Option<String>,
}

impl JDBCDatabaseConnection for DerbyConnection {
    fn jdbc_connection_details(
        &self,
        unique_database_name: &str,
    ) -> Result<JDBCDatabaseConnectionDetails, crate::crd::database::Error> {
        let location = self
            .location
            .clone()
            .unwrap_or_else(|| format!("/tmp/derby/{unique_database_name}/derby.db"));
        let connection_uri = format!("jdbc:derby:{location};create=true",);
        let connection_uri = connection_uri.parse().context(ParseConnectionUrlSnafu)?;

        Ok(JDBCDatabaseConnectionDetails {
            // Sadly the Derby driver class name is a bit complicated, e.g. for HMS up to 4.1.x we used
            // "org.apache.derby.jdbc.EmbeddedDriver",
            // for HMS 4.2.x we used "org.apache.derby.iapi.jdbc.AutoloadedDriver".
            driver: "org.apache.derby.jdbc.EmbeddedDriver".to_owned(),
            connection_uri,
            username_env: None,
            password_env: None,
        })
    }
}
