use std::str::FromStr;

use super::{
    rbac::RbacResourceNames,
    types::{
        kubernetes::{ClusterRoleName, DeploymentName, RoleBindingName, ServiceAccountName},
        operator::{ClusterName, ProductName},
    },
};

const SUFFIX: &str = "-agent";

/// Type-safe names for the resources of a product agent.
pub struct AgentResourceNames {
    pub cluster_name: ClusterName,
    pub product_name: ProductName,
}

impl AgentResourceNames {
    pub fn deployment_name(&self) -> DeploymentName {
        // compile-time checks
        const _: () = assert!(
            ClusterName::MAX_LENGTH + SUFFIX.len() <= DeploymentName::MAX_LENGTH,
            "The string `<cluster_name>-agent` must not exceed the limit of Deployment names."
        );
        let _ = ClusterName::IS_RFC_1035_LABEL_NAME;

        DeploymentName::from_str(&self.name()).expect("should be a valid Deployment name")
    }

    fn name(&self) -> String {
        let cluster_name = &self.cluster_name;
        format!("{cluster_name}{SUFFIX}")
    }
}

impl RbacResourceNames for AgentResourceNames {
    fn service_account_name(&self) -> ServiceAccountName {
        // compile-time checks
        const _: () = assert!(
            ClusterName::MAX_LENGTH + SUFFIX.len() <= ServiceAccountName::MAX_LENGTH,
            "The string `<cluster_name>-agent` must not exceed the limit of ServiceAccount names."
        );
        let _ = ClusterName::IS_RFC_1123_SUBDOMAIN_NAME;

        ServiceAccountName::from_str(&self.name()).expect("should be a valid ServiceAccount name")
    }

    fn role_binding_name(&self) -> RoleBindingName {
        // compile-time checks
        const _: () = assert!(
            ClusterName::MAX_LENGTH + SUFFIX.len() <= RoleBindingName::MAX_LENGTH,
            "The string `<cluster_name>-agent` must not exceed the limit of RoleBinding names."
        );
        let _ = ClusterName::IS_RFC_1123_SUBDOMAIN_NAME;

        RoleBindingName::from_str(&self.name()).expect("should be a valid RoleBinding name")
    }

    fn cluster_role_name(&self) -> ClusterRoleName {
        const CLUSTER_ROLE_SUFFIX: &str = "-agent-clusterrole";

        // compile-time checks
        const _: () = assert!(
            ProductName::MAX_LENGTH + CLUSTER_ROLE_SUFFIX.len() <= ClusterRoleName::MAX_LENGTH,
            "The string `<product_name>-agent-clusterrole` must not exceed the limit of cluster role names."
        );
        let _ = ProductName::IS_RFC_1123_SUBDOMAIN_NAME;

        let product_name = &self.product_name;
        ClusterRoleName::from_str(&format!("{product_name}{CLUSTER_ROLE_SUFFIX}"))
            .expect("should be a valid cluster role name")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_resource_names() {
        let resource_names = AgentResourceNames {
            cluster_name: ClusterName::from_str_unsafe("my-cluster"),
            product_name: ProductName::from_str_unsafe("my-product"),
        };

        assert_eq!(
            DeploymentName::from_str_unsafe("my-cluster-agent"),
            resource_names.deployment_name()
        );
        assert_eq!(
            ServiceAccountName::from_str_unsafe("my-cluster-agent"),
            resource_names.service_account_name()
        );
        assert_eq!(
            RoleBindingName::from_str_unsafe("my-cluster-agent"),
            resource_names.role_binding_name()
        );
        assert_eq!(
            ClusterRoleName::from_str_unsafe("my-product-agent-clusterrole"),
            resource_names.cluster_role_name()
        );
    }
}
