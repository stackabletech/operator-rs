use kube::{Resource, ResourceExt};
use snafu::{ResultExt, Snafu};

use crate::{
    builder::meta::ObjectMetaBuilder,
    k8s_openapi::api::{
        core::v1::ServiceAccount,
        rbac::v1::{RoleBinding, RoleRef, Subject},
    },
    kvp::Labels,
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("failed to set owner reference from resource for ServiceAccount {name:?}"))]
    ServiceAccountOwnerReferenceFromResource {
        source: crate::builder::meta::Error,
        name: String,
    },

    #[snafu(display("failed to set owner reference from resource Role Binding {name:?}"))]
    RoleBindingOwnerReferenceFromResource {
        source: crate::builder::meta::Error,
        name: String,
    },
}

/// Build RBAC objects for the product workloads.
/// The names of the service account and role binding match the following patterns:
/// - `{resource_name}-serviceaccount`
/// - `{resource_name}-rolebinding`
///
/// A previous version of this function used the `product_name` instead of the `resource_name`,
/// but this caused conflicts when deploying multiple instances of a product in the same namespace.
/// See <https://stackable.atlassian.net/browse/SUP-148> for more details.
///
/// The service account is bound to a cluster role named `{product_name}-clusterrole` which
/// must already exist.
pub fn build_rbac_resources<T: Clone + Resource<DynamicType = ()>>(
    resource: &T,
    product_name: &str,
    labels: Labels,
) -> Result<(ServiceAccount, RoleBinding)> {
    let sa_name = service_account_name(&resource.name_any());
    let service_account = ServiceAccount {
        metadata: ObjectMetaBuilder::new()
            .name_and_namespace(resource)
            .name(sa_name.clone())
            .ownerreference_from_resource(resource, None, Some(true))
            .with_context(|_| ServiceAccountOwnerReferenceFromResourceSnafu {
                name: sa_name.clone(),
            })?
            .with_labels(labels.clone())
            .build(),
        ..ServiceAccount::default()
    };

    let role_binding = RoleBinding {
        metadata: ObjectMetaBuilder::new()
            .name_and_namespace(resource)
            .name(role_binding_name(&resource.name_any()))
            .ownerreference_from_resource(resource, None, Some(true))
            .context(RoleBindingOwnerReferenceFromResourceSnafu {
                name: resource.name_any(),
            })?
            .with_labels(labels)
            .build(),
        role_ref: RoleRef {
            kind: "ClusterRole".to_string(),
            name: format!("{product_name}-clusterrole"),
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
/// This is private because operators should not use this function to calculate names for
/// serviceAccount objects, but rather read the name from the objects returned by
/// `build_rbac_resources` if they need the name.
fn service_account_name(rbac_prefix: &str) -> String {
    format!("{rbac_prefix}-serviceaccount")
}

/// Generate the role binding name.
/// The `rbac_prefix` is meant to be the product name, for example: zookeeper, airflow, etc.
/// This is private because operators should not use this function to calculate names for
/// roleBinding objects, but rather read the name from the objects returned by
/// `build_rbac_resources` if they need the name.
fn role_binding_name(rbac_prefix: &str) -> String {
    format!("{rbac_prefix}-rolebinding")
}

#[cfg(test)]
mod tests {
    use kube::CustomResource;
    use schemars::{self, JsonSchema};
    use serde::{Deserialize, Serialize};

    use crate::{
        commons::rbac::{build_rbac_resources, role_binding_name, service_account_name},
        kvp::Labels,
    };

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
    fn build() {
        let cluster = build_test_resource();
        let (rbac_sa, rbac_rolebinding) =
            build_rbac_resources(&cluster, RESOURCE_NAME, Labels::new()).unwrap();

        assert_eq!(
            Some(service_account_name(CLUSTER_NAME)),
            rbac_sa.metadata.name,
            "service account does not match"
        );
        assert_eq!(
            Some(format!("{CLUSTER_NAME}-ns")),
            rbac_sa.metadata.namespace,
            "namespace does not match"
        );

        assert_eq!(
            Some(role_binding_name(CLUSTER_NAME)),
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
