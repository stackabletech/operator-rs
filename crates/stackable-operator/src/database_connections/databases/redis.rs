use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    commons::networking::HostName,
    database_connections::{
        TemplatingMechanism,
        drivers::celery::{CeleryDatabaseConnection, CeleryDatabaseConnectionDetails},
        helpers::username_and_password_envs,
    },
};

/// Connection settings for a [Redis](https://redis.io/) instance.
///
/// Redis is commonly used as a Celery message broker or result backend (e.g. for Apache Airflow).
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisConnection {
    /// Hostname or IP address of the Redis server.
    pub host: HostName,

    /// Port the Redis server is listening on. Defaults to `6379`.
    #[serde(default = "RedisConnection::default_port")]
    pub port: u16,

    /// Numeric index of the Redis logical database to use. Defaults to `0`.
    ///
    /// Redis supports multiple logical databases within a single instance, identified by an
    /// integer index. Database `0` is the default.
    #[serde(default = "RedisConnection::default_database_id")]
    pub database_id: u16,

    /// Name of a Secret containing the `username` and `password` keys used to authenticate
    /// against the Redis server.
    pub credentials_secret: String,
}

impl RedisConnection {
    fn default_port() -> u16 {
        6379
    }

    fn default_database_id() -> u16 {
        0
    }
}

impl CeleryDatabaseConnection for RedisConnection {
    fn celery_connection_details_with_templating(
        &self,
        unique_database_name: &str,
        templating_mechanism: &TemplatingMechanism,
    ) -> CeleryDatabaseConnectionDetails {
        let Self {
            host,
            port,
            database_id,
            credentials_secret,
        } = self;
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, credentials_secret);
        let username_env_name = &username_env.name;
        let password_env_name = &password_env.name;

        let uri_template = match templating_mechanism {
            TemplatingMechanism::ConfigUtils => format!(
                "redis://${{env:{username_env_name}}}:${{env:{password_env_name}}}@{host}:{port}/{database_id}",
            ),
            TemplatingMechanism::BashEnvSubstitution => format!(
                "redis://${{{username_env_name}}}:${{{password_env_name}}}@{host}:{port}/{database_id}",
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

    const UNIQUE_DATABASE_NAME: &str = "WORKER_QUEUE";

    #[test]
    fn test_minimal_example() {
        let redis_connection: RedisConnection = serde_yaml::from_str(
            "
            host: my-redis
            port: 42
            databaseId: 13
            credentialsSecret: redis-credentials
            ",
        )
        .expect("invalid test input");
        let celery_connection_details =
            redis_connection.celery_connection_details(UNIQUE_DATABASE_NAME);
        assert_eq!(
            celery_connection_details.uri_template,
            "redis://${env:WORKER_QUEUE_DATABASE_USERNAME}:${env:WORKER_QUEUE_DATABASE_PASSWORD}@my-redis:42/13"
        );
        assert!(celery_connection_details.username_env.is_some());
        assert!(celery_connection_details.password_env.is_some());
        assert!(celery_connection_details.generic_uri_var.is_none());
    }
}
