use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    builder::pod::container::ContainerBuilder,
    crd::database::{
        databases::{postgresql::PostgresqlConnection, redis::RedisConnection},
        drivers::{
            celery::{CeleryDatabaseConnection, GenericCeleryDatabaseConnection},
            jdbc::{GenericJDBCDatabaseConnection, JDBCDatabaseConnection},
        },
    },
};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DummyJDBCConnection {
    Postgresql(PostgresqlConnection),
    #[allow(unused)]
    Generic(GenericJDBCDatabaseConnection),
}

impl DummyJDBCConnection {
    fn as_jdbc_database_connection(&self) -> &dyn JDBCDatabaseConnection {
        match self {
            Self::Postgresql(p) => p,
            Self::Generic(g) => g,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DummyCeleryConnection {
    Redis(RedisConnection),
    #[allow(unused)]
    Generic(GenericCeleryDatabaseConnection),
}

impl DummyCeleryConnection {
    fn as_celery_database_connection(&self) -> &dyn CeleryDatabaseConnection {
        match self {
            Self::Redis(r) => r,
            Self::Generic(g) => g,
        }
    }
}

#[test]
fn test_dummy_jdbc_database_usage() {
    // Set up test data
    let dummy_jdbc_connection = DummyJDBCConnection::Postgresql(PostgresqlConnection {
        host: "my-database".parse().expect("static host is always valid"),
        port: 1234,
        database: "my_schema".to_owned(),
        credentials_secret: "my-credentials".to_owned(),
        parameters: BTreeMap::new(),
    });
    // Apply actual config
    let jdbc_connection_details = dummy_jdbc_connection
        .as_jdbc_database_connection()
        .jdbc_connection_details("persistence")
        .unwrap();
    let mut container_builder = ContainerBuilder::new("my-container").unwrap();
    jdbc_connection_details.add_to_container(&mut container_builder);
    let container = container_builder.build();

    assert_eq!(jdbc_connection_details.driver, "org.postgresql.Driver");
    assert_eq!(
        jdbc_connection_details.connection_uri.to_string(),
        "jdbc:postgresql://my-database:1234/my_schema"
    );
    assert_eq!(
        container
            .env
            .unwrap()
            .iter()
            .map(|env| &env.name)
            .collect::<Vec<_>>(),
        vec![
            "PERSISTENCE_DATABASE_USERNAME",
            "PERSISTENCE_DATABASE_PASSWORD"
        ]
    );
}

#[test]
fn test_dummy_celery_database_usage() {
    // Set up test data
    let dummy_celery_connection = DummyCeleryConnection::Generic(GenericCeleryDatabaseConnection {
        uri_secret: "my-celery-db".to_owned(),
    });
    // Apply actual config
    let celery_connection_details = dummy_celery_connection
        .as_celery_database_connection()
        .celery_connection_details("worker_queue");
    let mut container_builder = ContainerBuilder::new("my-container").unwrap();
    celery_connection_details.add_to_container(&mut container_builder);
    let container = container_builder.build();

    assert_eq!(
        celery_connection_details.uri_template,
        "${env:WORKER_QUEUE_DATABASE_URI}"
    );
    assert_eq!(
        container
            .env
            .unwrap()
            .iter()
            .map(|env| &env.name)
            .collect::<Vec<_>>(),
        vec!["WORKER_QUEUE_DATABASE_URI"]
    );
}
