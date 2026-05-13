use std::str::FromStr;

use super::types::{
    kubernetes::{ConfigMapName, ListenerName, ServiceName, StatefulSetName},
    operator::{ClusterName, RoleGroupName, RoleName},
};
use crate::attributed_string_type;

attributed_string_type! {
    QualifiedRoleGroupName,
    "A qualified role group name consisting of the cluster name, role name and role-group name. It is a valid label name as defined in RFC 1035 that can be used e.g. as a name for a Service or a StatefulSet.",
    "opensearch-nodes-default",
    // Suffixes are added to produce resource names. According compile-time checks ensure that
    // max_length cannot be set higher.
    (max_length = 52),
    is_rfc_1035_label_name,
    is_valid_label_value
}

/// Type-safe names for role-group resources
pub struct ResourceNames {
    pub cluster_name: ClusterName,
    pub role_name: RoleName,
    pub role_group_name: RoleGroupName,
}

impl ResourceNames {
    /// Creates a qualified role group name in the format
    /// `<cluster_name>-<role_name>-<role_group_name>`
    fn qualified_role_group_name(&self) -> QualifiedRoleGroupName {
        // compile-time checks
        const _: () = assert!(
            ClusterName::MAX_LENGTH
                + 1 // dash
                + RoleName::MAX_LENGTH
                + 1 // dash
                + RoleGroupName::MAX_LENGTH
                <= QualifiedRoleGroupName::MAX_LENGTH,
            "The string `<cluster_name>-<role_name>-<role_group_name>` must not exceed the limit \
            of RFC 1035 label names."
        );
        // qualified_role_group_name is only an RFC 1035 label name if it starts with an
        // alphabetic character, therefore cluster_name must also be an RFC 1035 label name.
        // role_name and role_group_name and the middle of the qualified_role_group_name can
        // be RFC 1123 label names because digits are allowed there.
        let _ = ClusterName::IS_RFC_1035_LABEL_NAME;
        let _ = RoleName::IS_RFC_1123_LABEL_NAME;
        let _ = RoleGroupName::IS_RFC_1123_LABEL_NAME;

        QualifiedRoleGroupName::from_str(&format!(
            "{}-{}-{}",
            self.cluster_name, self.role_name, self.role_group_name,
        ))
        .expect("should be a valid QualifiedRoleGroupName")
    }

    pub fn role_group_config_map(&self) -> ConfigMapName {
        // compile-time check
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= ConfigMapName::MAX_LENGTH,
            "The string `<cluster_name>-<role_name>-<role_group_name>` must not exceed the limit of \
            ConfigMap names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_SUBDOMAIN_NAME;

        ConfigMapName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid ConfigMap name")
    }

    pub fn stateful_set_name(&self) -> StatefulSetName {
        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= StatefulSetName::MAX_LENGTH,
            "The string `<cluster_name>-<role_name>-<role_group_name>` must not exceed the \
            limit of StatefulSet names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_LABEL_NAME;
        let _ = QualifiedRoleGroupName::IS_VALID_LABEL_VALUE;

        StatefulSetName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid StatefulSet name")
    }

    pub fn headless_service_name(&self) -> ServiceName {
        const SUFFIX: &str = "-headless";

        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH + SUFFIX.len() <= ServiceName::MAX_LENGTH,
            "The string `<cluster_name>-<role_name>-<role_group_name>-headless` must not exceed the \
            limit of Service names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1035_LABEL_NAME;
        let _ = QualifiedRoleGroupName::IS_VALID_LABEL_VALUE;

        ServiceName::from_str(&format!("{}{SUFFIX}", self.qualified_role_group_name()))
            .expect("should be a valid Service name")
    }

    pub fn listener_name(&self) -> ListenerName {
        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= ListenerName::MAX_LENGTH,
            "The string `<cluster_name>-<role_name>-<role_group_name>` must not exceed the limit of \
            Listener names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_SUBDOMAIN_NAME;

        ListenerName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid Listener name")
    }
}

#[cfg(test)]
mod tests {
    use super::{ClusterName, RoleGroupName, RoleName};
    use crate::framework::{
        role_group_utils::{QualifiedRoleGroupName, ResourceNames},
        types::kubernetes::{ConfigMapName, ListenerName, ServiceName, StatefulSetName},
    };

    #[test]
    fn test_resource_names() {
        QualifiedRoleGroupName::test_example();

        let resource_names = ResourceNames {
            cluster_name: ClusterName::from_str_unsafe("test-cluster"),
            role_name: RoleName::from_str_unsafe("data-nodes"),
            role_group_name: RoleGroupName::from_str_unsafe("ssd-storage"),
        };

        assert_eq!(
            QualifiedRoleGroupName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.qualified_role_group_name()
        );
        assert_eq!(
            ConfigMapName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.role_group_config_map()
        );
        assert_eq!(
            StatefulSetName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.stateful_set_name()
        );
        assert_eq!(
            ServiceName::from_str_unsafe("test-cluster-data-nodes-ssd-storage-headless"),
            resource_names.headless_service_name()
        );
        assert_eq!(
            ListenerName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.listener_name()
        );
    }
}
