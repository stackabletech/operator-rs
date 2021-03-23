use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::Meta;
use stackable_operator::crd::{ensure_crd_created, exists, wait_deleted};
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

#[tokio::test]
#[ignore = "Tests depending on Kubernetes are not ran by default"]
async fn k8s_test_test_ensure_crd_created() {
    let client = client::create_client(None)
        .await
        .expect("KUBECONFIG variable must be configured.");

    tokio::time::timeout(
        Duration::from_secs(30),
        ensure_crd_created::<TestCrd>(client.clone()),
    )
    .await
    .expect("CRD not created in time")
    .expect("Error while creating CRD");

    exists::<TestCrd>(client.clone())
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

    tokio::time::timeout(
        Duration::from_secs(30),
        wait_deleted::<TestCrd>(client.clone()),
    )
    .await
    .expect("Expected CRD to be deleted")
    .expect("");

    assert!(!exists::<TestCrd>(client.clone())
        .await
        .expect("CRD should exist"))
}
