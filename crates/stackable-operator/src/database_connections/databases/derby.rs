use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::{
    database_connections::{
        TemplatingMechanism,
        drivers::jdbc::{JdbcDatabaseConnection, JdbcDatabaseConnectionDetails},
    },
    utils::OptionExt as _,
};

/// Sadly the Derby driver class name is a bit complicated, e.g. for HMS up to 4.1.x we used
/// `org.apache.derby.jdbc.EmbeddedDriver`, for HMS 4.2.x we used
/// `org.apache.derby.iapi.jdbc.AutoloadedDriver`.
pub const DERBY_JDBC_DRIVER_CLASS: &str = "org.apache.derby.jdbc.EmbeddedDriver";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse connection URL"))]
    ParseConnectionUrl { source: url::ParseError },

    #[snafu(display("invalid derby database location, likely as it contains non-utf8 characters"))]
    NonUtf8Location { location: PathBuf },
}

/// Connection settings for an embedded [Apache Derby](https://db.apache.org/derby/) database.
///
/// Derby is an embedded, file-based Java database engine that requires no separate server process.
/// It is typically used for development, testing, or as a lightweight metastore backend (e.g. for
/// Apache Hive).
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerbyConnection {
    /// Path on the filesystem where Derby stores its database files.
    ///
    /// If not specified, defaults to `/tmp/derby/{unique_database_name}/derby.db`.
    /// The `{unique_database_name}` part is automatically handled by the operator and is added to
    /// prevent clashing database files. The `create=true` flag is always appended to the JDBC URL,
    /// so the database is created automatically if it does not yet exist at this location.
    pub location: Option<PathBuf>,
}

impl JdbcDatabaseConnection for DerbyConnection {
    fn jdbc_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        _templating_mechanism: &TemplatingMechanism,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::database_connections::Error> {
        let location = self.location(unique_database_name)?;
        let connection_url = format!("jdbc:derby:{location};create=true")
            .parse()
            .context(ParseConnectionUrlSnafu)?;

        Ok(JdbcDatabaseConnectionDetails {
            driver: DERBY_JDBC_DRIVER_CLASS.to_owned(),
            connection_url,
            username_env: None,
            password_env: None,
        })
    }
}

impl DerbyConnection {
    /// Returns the JDBC connection URL in a format such as
    /// `jdbc:derby://localhost:1527//opt/var/druid_state/derby;create=true`.
    ///
    /// E.g. according to the [Druid docs](https://druid.apache.org/docs/latest/design/metadata-storage/#derby)
    /// we should configure something like
    /// `jdbc:derby://localhost:1527//opt/var/druid_state/derby;create=true`.
    ///
    /// Druid actually starts a Derby instance, which listens on `127.0.0.1:1527`. The schema seems
    /// to be the filesystem location.
    ///
    /// As stackable-operator generates a (correct) URL in the form
    /// `jdbc:derby:/tmp/foo/bar.db;create=true`, this function converts it to
    /// `jdbc:derby://dummy-host-for-druid:1234/tmp/foo/bar.db;create=true`
    pub fn jdbc_connection_details_with_host_part(
        &self,
        unique_database_name: &str,
        host_part: &str,
    ) -> Result<JdbcDatabaseConnectionDetails, crate::database_connections::Error> {
        let location = self.location(unique_database_name)?;
        let connection_url = format!("jdbc:derby://{host_part}/{location};create=true")
            .parse()
            .context(ParseConnectionUrlSnafu)?;

        Ok(JdbcDatabaseConnectionDetails {
            driver: DERBY_JDBC_DRIVER_CLASS.to_owned(),
            connection_url,
            username_env: None,
            password_env: None,
        })
    }

    /// Returns the configured [`Self::location`] or a sensible default value
    fn location(
        &self,
        unique_database_name: &str,
    ) -> Result<String, crate::database_connections::Error> {
        let location = self.location.as_ref_or_else(|| {
            PathBuf::from(format!("/tmp/derby/{unique_database_name}/derby.db"))
        });
        Ok(location
            .to_str()
            .with_context(|| NonUtf8LocationSnafu {
                location: location.to_path_buf(),
            })?
            .to_owned())
    }
}
