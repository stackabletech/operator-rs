use crate::builder::ObjectMetaBuilder;
use crate::k8s_openapi::api::core::v1::ServiceAccount;
use crate::k8s_openapi::api::rbac::v1::{RoleBinding, RoleRef, Subject};
use kube::{Resource, ResourceExt};

/// Build RBAC objects for the product workloads.
/// The `rbac_prefix` is meant to be the product name, for example: zookeeper, airflow, etc.
/// and it is a assumed that a ClusterRole named `{rbac_prefix}-clusterrole` exists.
pub fn build_rbac_resources<T: Resource>(
    resource: &T,
    rbac_prefix: &str,
) -> (ServiceAccount, RoleBinding) {
    let sa_name = format!("{rbac_prefix}-sa");
    let service_account = ServiceAccount {
        metadata: ObjectMetaBuilder::new()
            .name_and_namespace(resource)
            .name(sa_name.clone())
            .build(),
        ..ServiceAccount::default()
    };

    let role_binding = RoleBinding {
        metadata: ObjectMetaBuilder::new()
            .name_and_namespace(resource)
            .name(format!("{rbac_prefix}-rolebinding"))
            .build(),
        role_ref: RoleRef {
            kind: "ClusterRole".to_string(),
            name: format!("{rbac_prefix}-clusterrole"),
            api_group: "rbac.authorization.k8s.io".to_string(),
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".to_string(),
            name: sa_name,
            namespace: resource.namespace(),
            ..Subject::default()
        }]),
    };

    (service_account, role_binding)
}

#[cfg(test)]
mod tests {
    use crate::commons::rbac::build_rbac_resources;
    use kube::CustomResource;
    use schemars::{self, JsonSchema};
    use serde::{Deserialize, Serialize};

    const CLUSTER_NAME: &str = "simple-cluster";
    const RESOURCE_NAME: &str = "test-resource";

    #[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
    #[kube(group = "test", version = "v1", kind = "TestCluster", namespaced)]
    pub struct ClusterSpec {
        test: u8,
    }

    fn build_test_resource() -> TestCluster {
        serde_yaml::from_str(&format!(
            "
            apiVersion: test/v1
            kind: TestCluster
            metadata:
              name: {CLUSTER_NAME}
              namespace: {CLUSTER_NAME}-ns
            spec:
              test: 100
            "
        ))
        .unwrap()
    }

    #[test]
    fn test_build_rbac() {
        let cluster = build_test_resource();
        let (rbac_sa, rbac_rolebinding) = build_rbac_resources(&cluster, RESOURCE_NAME);

        assert_eq!(
            Some(format!("{RESOURCE_NAME}-sa")),
            rbac_sa.metadata.name,
            "service account does not match"
        );
        assert_eq!(
            Some(format!("{CLUSTER_NAME}-ns")),
            rbac_sa.metadata.namespace,
            "namespace does not match"
        );

        assert_eq!(
            Some(format!("{RESOURCE_NAME}-rolebinding")),
            rbac_rolebinding.metadata.name,
            "rolebinding does not match"
        );
        assert_eq!(
            Some(format!("{CLUSTER_NAME}-ns")),
            rbac_rolebinding.metadata.namespace,
            "namespace does not match"
        );

        assert_eq!(
            format!("{RESOURCE_NAME}-clusterrole").to_string(),
            rbac_rolebinding.role_ref.name,
            "role_ref does not match"
        );
    }
}
