use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::core::ResourceExt;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use stackable_operator::client;
use stackable_operator::client::Client;
use stackable_operator::crd::{
    ensure_crd_created, wait_until_crds_present, NamedCustomResourceExt,
};

#[derive(Clone, CustomResource, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "zookeeper.stackable.tech",
    version = "v1",
    kind = "TestCrdStruct",
    shortname = "zk",
    namespaced
)]
struct TestCrd {}

impl NamedCustomResourceExt for TestCrdStruct {
    const RESOURCE_NAME: &'static str = "tests.stackable.tech";
}

#[derive(Clone, CustomResource, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "zookeeper.stackable.tech",
    version = "v1",
    kind = "TestCrd2Struct",
    shortname = "zk",
    namespaced
)]
struct TestCrd2 {}

impl NamedCustomResourceExt for TestCrd2Struct {
    const RESOURCE_NAME: &'static str = "tests2.stackable.tech";
}

async fn setup(client: &Client) {
    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrdStruct>(&client),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");

    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrd2Struct>(&client),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");
}

async fn tear_down(client: &Client) {
    let mut operations = vec![];

    for crd_name in &[TestCrdStruct::RESOURCE_NAME, TestCrd2Struct::RESOURCE_NAME] {
        if let Ok(crd) = client.get::<CustomResourceDefinition>(crd_name, None).await {
            operations.push(client.ensure_deleted(crd));
        }
    }

    let result = tokio::time::timeout(
        Duration::from_secs(30),
        futures::future::join_all(operations),
    )
    .await
    .unwrap_or_else(|_| panic!("Unable to cleanup, delete operation timed out!"));

    let failed_operations = result.iter().filter(|res| res.is_err()).collect::<Vec<_>>();

    if !failed_operations.is_empty() {
        panic!(
            "Failed to delete the following CRDs during cleanup: [{:?}]",
            failed_operations
        );
    }
}

#[tokio::test]
#[serial]
#[ignore = "Tests depending on Kubernetes are not ran by default"]
async fn k8s_test_wait_for_crds() {
    // TODO: Switch this to using TemporaryResource from the integration-test-commons crate
    let client = client::create_client(None)
        .await
        .expect("KUBECONFIG variable must be configured.");

    setup(&client).await;

    // Test waiting honors timeout
    let await_result = tokio::time::timeout(
        Duration::from_secs(30),
        wait_until_crds_present(
            &client,
            vec!["non_existing_crd_name"],
            Some(Duration::from_secs(10)),
        ),
    )
    .await
    .expect("Waiting for CRDs did not return within the configured timeout!");

    match await_result {
        Err(stackable_operator::error::Error::RequiredCrdsMissing { names }) => {
            assert_eq!(
                names,
                vec!["non_existing_crd_name".to_string()]
                    .into_iter()
                    .collect()
            )
        }
        _ => panic!("Did not get the expected error!"),
    }

    // Check that waiting returns promptly when all CRDs exist
    let await_result = tokio::time::timeout(
        Duration::from_secs(30),
        wait_until_crds_present(
            &client,
            vec![TestCrdStruct::RESOURCE_NAME],
            Some(Duration::from_secs(10)),
        ),
    )
    .await
    .expect("Checking for an existing CRD should have returned before the timeout!");

    match await_result {
        Ok(()) => {}
        Err(e) => panic!("Got error instead of expected Ok(()): [{:?}]", e),
    }

    // Check await returns an error when one of multiple expected CRDs is missing
    let await_result = tokio::time::timeout(
        Duration::from_secs(30),
        wait_until_crds_present(
            &client,
            vec![TestCrdStruct::RESOURCE_NAME, "MissingCrdName"],
            Some(Duration::from_secs(10)),
        ),
    )
    .await
    .expect("Waiting for CRDs did not return within the configured timeout!");

    match await_result {
        Err(stackable_operator::error::Error::RequiredCrdsMissing { names }) => {
            assert_eq!(
                names,
                vec!["MissingCrdName".to_string()].into_iter().collect()
            )
        }
        _ => panic!("Did not get the expected error!"),
    }

    // Check with two existing CRDs
    let await_result = tokio::time::timeout(
        Duration::from_secs(30),
        wait_until_crds_present(
            &client,
            vec![TestCrdStruct::RESOURCE_NAME, TestCrd2Struct::RESOURCE_NAME],
            Some(Duration::from_secs(10)),
        ),
    )
    .await
    .expect("Waiting for CRDs did not return within the configured timeout!");

    match await_result {
        Ok(()) => {}
        Err(e) => panic!("Got error instead of expected Ok(()): [{:?}]", e),
    }

    // Check with two existing and a two missing CRDs
    let await_result = tokio::time::timeout(
        Duration::from_secs(30),
        wait_until_crds_present(
            &client,
            vec![
                TestCrdStruct::RESOURCE_NAME,
                TestCrd2Struct::RESOURCE_NAME,
                "missing1",
                "missing2",
            ],
            Some(Duration::from_secs(10)),
        ),
    )
    .await
    .expect("Waiting for CRDs did not return within the configured timeout!");

    match await_result {
        Err(stackable_operator::error::Error::RequiredCrdsMissing { names }) => {
            assert_eq!(
                names,
                vec!["missing1".to_string(), "missing2".to_string()]
                    .into_iter()
                    .collect()
            )
        }
        _ => panic!("Did not get the expected error!"),
    }

    tear_down(&client).await;
}

#[tokio::test]
#[serial]
#[ignore = "Tests depending on Kubernetes are not ran by default"]
async fn k8s_test_test_ensure_crd_created() {
    let client = client::create_client(None)
        .await
        .expect("KUBECONFIG variable must be configured.");

    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrdStruct>(&client),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");

    client
        .exists::<CustomResourceDefinition>(TestCrdStruct::RESOURCE_NAME, None)
        .await
        .expect("CRD should be created");
    let created_crd: CustomResourceDefinition = client
        .get(TestCrdStruct::RESOURCE_NAME.as_ref(), None)
        .await
        .unwrap();
    assert_eq!(TestCrdStruct::RESOURCE_NAME, created_crd.name());

    tear_down(&client).await;
}
