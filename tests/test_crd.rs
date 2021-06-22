use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::core::ResourceExt;
use stackable_operator::crd::{ensure_crd_created, wait_until_crds_present};
use stackable_operator::{client, Crd};

struct TestCrd {}

impl Crd for TestCrd {
    const RESOURCE_NAME: &'static str = "tests.stackable.tech";
    const CRD_DEFINITION: &'static str = r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: tests.stackable.tech
spec:
  group: stackable.tech
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
  scope: Namespaced
  names:
    plural: tests
    singular: test
    kind: Test
"#;
}

struct TestCrd2 {}

impl Crd for TestCrd2 {
    const RESOURCE_NAME: &'static str = "tests2.stackable.tech";
    const CRD_DEFINITION: &'static str = r#"
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: tests2.stackable.tech
spec:
  group: stackable.tech
  versions:
    - name: v1
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
  scope: Namespaced
  names:
    plural: tests2
    singular: test2
    kind: Test2
"#;
}

#[tokio::test]
#[ignore = "Tests depending on Kubernetes are not ran by default"]
async fn k8s_test_test_ensure_crd_created() {
    let client = client::create_client(None)
        .await
        .expect("KUBECONFIG variable must be configured.");

    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrd>(&client),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");

    client
        .exists::<CustomResourceDefinition>(TestCrd::RESOURCE_NAME, None)
        .await
        .expect("CRD should be created");
    let created_crd: CustomResourceDefinition = client
        .get(TestCrd::RESOURCE_NAME.as_ref(), None)
        .await
        .unwrap();
    assert_eq!(TestCrd::RESOURCE_NAME, created_crd.name());

    client
        .delete(&created_crd)
        .await
        .expect("TestCrd not deleted");
    assert!(client
        .exists::<CustomResourceDefinition>(TestCrd::RESOURCE_NAME, None)
        .await
        .expect("CRD should be created"))
}

#[tokio::test]
#[ignore = "Tests depending on Kubernetes are not ran by default"]
async fn k8s_test_wait_for_crds() {
    // TODO: Switch this to using TemporaryResource from the integration-test-commons crate
    let client = client::create_client(None)
        .await
        .expect("KUBECONFIG variable must be configured.");

    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrd>(&client),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");

    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrd2>(&client),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");

    // Give k8s some time to perform the deletion
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Test waiting honors timeout
    let await_result = tokio::time::timeout(
        Duration::from_secs(30),
        wait_until_crds_present(
            &client,
            vec!["non_existing_crd_name"],
            Some(Duration::from_secs(1)),
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
            vec![TestCrd::RESOURCE_NAME],
            Some(Duration::from_secs(1)),
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
            vec![TestCrd::RESOURCE_NAME, "MissingCrdName"],
            Some(Duration::from_secs(1)),
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
            vec![TestCrd::RESOURCE_NAME, TestCrd2::RESOURCE_NAME],
            Some(Duration::from_secs(1)),
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
                TestCrd::RESOURCE_NAME,
                TestCrd2::RESOURCE_NAME,
                "missing1",
                "missing2",
            ],
            Some(Duration::from_secs(1)),
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

    // Cleanup
    for crd_name in &[TestCrd::RESOURCE_NAME, TestCrd2::RESOURCE_NAME] {
        if let Ok(crd) = client.get::<CustomResourceDefinition>(crd_name, None).await {
            // Ensure CRD is not present
            client
                .delete(&crd)
                .await
                .unwrap_or_else(|_| panic!("Unable to delete CRD [{}]", crd_name));
        }
    }
}
