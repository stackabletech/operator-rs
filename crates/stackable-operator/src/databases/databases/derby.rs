use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::databases::{
    TemplatingMechanism,
    drivers::jdbc::{JdbcDatabaseConnection, JdbcDatabaseConnectionDetails},
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse connection URL"))]
    ParseConnectionUrl { source: url::ParseError },
}

/// Connection settings for an embedded [Apache Derby](https://db.apache.org/derby/) database.
///
/// Derby is an embedded, file-based Java database engine that requires no separate server process.
/// It is typically used for development, testing, or as a lightweight metastore backend (e.g. for
/// Apache Hive).
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerbyConnection {
    /// Path on the filesystem where Derby stores its database files.
    ///
    /// If not specified, defaults to `/tmp/derby/{unique_database_name}/derby.db`.
    /// The `{unique_database_name}` part is automatically handled by the operator and is added to
    /// prevent clashing database files. The `create=true` flag is always appended to the JDBC URL,
    /// so the database is created automatically if it does not yet exist at this location.
    pub location: Option<String>,
}

impl JdbcDatabaseConnection for DerbyConnection {
    fn jdbc_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        _templating_mechanism: &TemplatingMechanism,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::databases::Error> {
        let location = self
            .location
            .clone()
            .unwrap_or_else(|| format!("/tmp/derby/{unique_database_name}/derby.db"));
        let connection_uri = format!("jdbc:derby:{location};create=true",);
        let connection_uri = connection_uri.parse().context(ParseConnectionUrlSnafu)?;

        Ok(JdbcDatabaseConnectionDetails {
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
