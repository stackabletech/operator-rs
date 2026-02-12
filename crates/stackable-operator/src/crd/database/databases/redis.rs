use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    commons::networking::HostName,
    crd::database::{
        drivers::celery::{CeleryDatabaseConnection, CeleryDatabaseConnectionDetails},
        helpers::username_and_password_envs,
    },
};

/// TODO docs
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisConnection {
    /// TODO docs
    pub host: HostName,

    /// TODO docs
    #[serde(default = "RedisConnection::default_port")]
    pub port: u16,

    /// TODO docs
    #[serde(default = "RedisConnection::default_database_id")]
    pub database_id: u16,

    /// TODO docs
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
    fn celery_connection_details(
        &self,
        unique_database_name: &str,
    ) -> CeleryDatabaseConnectionDetails {
        let Self {
            host,
            port,
            database_id,
            credentials_secret,
        } = self;
        let (username_env, password_env) =
            username_and_password_envs(unique_database_name, credentials_secret);

        let uri_template = format!(
            "redis://${{{username_env_name}}}:${{{password_env_name}}}@{host}:{port}/{database_id}",
            username_env_name = username_env.name,
            password_env_name = password_env.name,
        );
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
            "redis://${WORKER_QUEUE_DATABASE_USERNAME}:${WORKER_QUEUE_DATABASE_PASSWORD}@my-redis:42/13"
        );
        assert!(celery_connection_details.username_env.is_some());
        assert!(celery_connection_details.password_env.is_some());
        assert!(celery_connection_details.generic_uri_var.is_none());
    }
}
