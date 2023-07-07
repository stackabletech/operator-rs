use std::collections::BTreeMap;

use kube::{Resource, ResourceExt};

use crate::{
    builder::ObjectMetaBuilder,
    error::OperatorResult,
    k8s_openapi::api::{
        core::v1::ServiceAccount,
        rbac::v1::{RoleBinding, RoleRef, Subject},
    },
    types::Label,
};

/// Build RBAC objects for the product workloads.
/// The `rbac_prefix` is meant to be the product name, for example: zookeeper, airflow, etc.
/// and it is a assumed that a ClusterRole named `{rbac_prefix}-clusterrole` exists.
pub fn build_rbac_resources<T: Clone + Resource<DynamicType = ()>>(
    resource: &T,
    rbac_prefix: &str,
    labels: BTreeMap<String, Label>,
) -> OperatorResult<(ServiceAccount, RoleBinding)> {
    let sa_name = service_account_name(rbac_prefix);
    let service_account = ServiceAccount {
        metadata: ObjectMetaBuilder::new()
            .name_and_namespace(resource)
            .name(sa_name.clone())
            .ownerreference_from_resource(resource, None, Some(true))?
            .with_labels(labels.clone())
            .build(),
        ..ServiceAccount::default()
    };

    let role_binding = RoleBinding {
        metadata: ObjectMetaBuilder::new()
            .name_and_namespace(resource)
            .name(role_binding_name(rbac_prefix))
            .ownerreference_from_resource(resource, None, Some(true))?
            .with_labels(labels)
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

    Ok((service_account, role_binding))
}

/// Generate the service account name.
/// The `rbac_prefix` is meant to be the product name, for example: zookeeper, airflow, etc.
pub fn service_account_name(rbac_prefix: &str) -> String {
    format!("{rbac_prefix}-serviceaccount")
}

/// Generate the role binding name.
/// The `rbac_prefix` is meant to be the product name, for example: zookeeper, airflow, etc.
pub fn role_binding_name(rbac_prefix: &str) -> String {
    format!("{rbac_prefix}-rolebinding")
}

#[cfg(test)]
mod tests {
    use crate::commons::rbac::{build_rbac_resources, role_binding_name, service_account_name};
    use kube::CustomResource;
    use schemars::{self, JsonSchema};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

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
              uid: 12345
            spec:
              test: 100
            "
        ))
        .unwrap()
    }

    #[test]
    fn test_build_rbac() {
        let cluster = build_test_resource();
        let (rbac_sa, rbac_rolebinding) =
            build_rbac_resources(&cluster, RESOURCE_NAME, BTreeMap::new()).unwrap();

        assert_eq!(
            Some(service_account_name(RESOURCE_NAME)),
            rbac_sa.metadata.name,
            "service account does not match"
        );
        assert_eq!(
            Some(format!("{CLUSTER_NAME}-ns")),
            rbac_sa.metadata.namespace,
            "namespace does not match"
        );

        assert_eq!(
            Some(role_binding_name(RESOURCE_NAME)),
            rbac_rolebinding.metadata.name,
            "rolebinding does not match"
        );
        assert_eq!(
            Some(format!("{CLUSTER_NAME}-ns")),
            rbac_rolebinding.metadata.namespace,
            "namespace does not match"
        );

        assert_eq!(
            format!("{RESOURCE_NAME}-clusterrole"),
            rbac_rolebinding.role_ref.name,
            "role_ref does not match"
        );
    }
}
