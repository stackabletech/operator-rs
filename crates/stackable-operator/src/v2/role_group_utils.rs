use std::str::FromStr;

use sha2::{Digest, Sha256};

use super::types::{
    kubernetes::{
        ConfigMapName, DaemonSetName, DeploymentName, ListenerName, ServiceName, StatefulSetName,
    },
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
    ///
    /// If the result would exceed the maximum length of qualified role group names, then it is
    /// truncated and a hash is appended. The maximum length of the cluster name is short enough,
    /// so that a part of the role name is always rendered. The role group name is barely used and
    /// often set to "default", so that the qualified role group name is still meaningful:
    ///
    /// ```rust
    /// # use std::str::FromStr;
    /// # use stackable_operator::v2::role_group_utils::ResourceNames;
    /// # use stackable_operator::v2::types::operator::{ClusterName, RoleGroupName, RoleName};
    ///
    /// let resource_names = ResourceNames {
    ///     cluster_name: ClusterName::from_str("an-exceptional-long-cluster-name").unwrap(),
    ///     role_name: RoleName::from_str("dagprocessor").unwrap(),
    ///     role_group_name: RoleGroupName::from_str("default").unwrap(),
    /// };
    ///
    /// assert_eq!(
    ///     "an-exceptional-long-cluster-name-dagprocessor-6cc08b",
    ///     resource_names.qualified_role_group_name().to_string()
    /// );
    /// ```
    pub fn qualified_role_group_name(&self) -> QualifiedRoleGroupName {
        // compile-time checks
        const HASH_LENGTH: usize = 6;

        // At least the cluster name should be short enough to not be replaced by the hash.
        const _: () = assert!(
            ClusterName::MAX_LENGTH
                + 1 // dash
                + HASH_LENGTH
                <= QualifiedRoleGroupName::MAX_LENGTH,
            "The string `<cluster_name>-<hash>` must not exceed the limit of qualified role group \
            names."
        );

        // qualified_role_group_name is only an RFC 1035 label name if it starts with an
        // alphabetic character, therefore cluster_name must also be an RFC 1035 label name.
        // role_name and role_group_name and the middle of the qualified_role_group_name can
        // be RFC 1123 label names because digits are allowed there.
        let _ = ClusterName::IS_RFC_1035_LABEL_NAME;
        let _ = RoleName::IS_RFC_1123_LABEL_NAME;
        let _ = RoleGroupName::IS_RFC_1123_LABEL_NAME;

        let concatenated_name = format!(
            "{}-{}-{}",
            self.cluster_name, self.role_name, self.role_group_name,
        );
        // `concatenated_name` contains only ASCII characters.
        let sanitized_name = Self::ensure_max_length(
            concatenated_name,
            QualifiedRoleGroupName::MAX_LENGTH,
            HASH_LENGTH,
        );

        QualifiedRoleGroupName::from_str(&sanitized_name)
            .expect("should be a valid QualifiedRoleGroupName")
    }

    /// Ensures that the given resource name does not exceed the given maximum length.
    /// If required, the resource name is truncated and a hex encoded hash is appended with a dash.
    ///
    /// # Panics
    ///
    /// Panics if `resource_name` contains non-ASCII characters or if
    /// `max_length < 1 /* character */ + 1 /* dash */ + hash_length`.
    ///
    /// Kubernetes object names cannot contain non-ASCII characters.
    fn ensure_max_length(resource_name: String, max_length: usize, hash_length: usize) -> String {
        assert!(resource_name.is_ascii());
        assert!(max_length >= 1 /* character */ + 1 /* dash */ + hash_length);

        if resource_name.len() <= max_length {
            resource_name
        } else if hash_length == 0 {
            let mut truncated_name = resource_name;
            truncated_name.truncate(max_length);
            truncated_name
        } else {
            let mut hash = format!("{:x}", Sha256::digest(resource_name.as_bytes()));
            hash.truncate(hash_length);

            let mut truncated_name = resource_name;
            // Truncate the name so that the hash can be appended without exceeding the maximum
            // length.
            truncated_name.truncate(max_length - hash_length);

            let last_char = truncated_name
                .pop()
                .expect("should be guaranteed by the assertion above");
            let second_to_last_char = truncated_name
                .pop()
                .expect("should be guaranteed by the assertion above");

            // If the truncated name already ends with a dash then do not add another one,
            // otherwise replace the last character with a dash.
            if second_to_last_char == '-' && last_char != '-' {
                format!("{truncated_name}{second_to_last_char}{hash}")
            } else {
                format!("{truncated_name}{second_to_last_char}-{hash}")
            }
        }
    }

    pub fn role_group_config_map(&self) -> ConfigMapName {
        // compile-time check
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= ConfigMapName::MAX_LENGTH,
            "The string `<qualified_role_group_name>` must not exceed the limit of ConfigMap names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_SUBDOMAIN_NAME;

        ConfigMapName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid ConfigMap name")
    }

    pub fn daemon_set_name(&self) -> DaemonSetName {
        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= DaemonSetName::MAX_LENGTH,
            "The string `<qualified_role_group_name>` must not exceed the limit of DaemonSet names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_SUBDOMAIN_NAME;

        DaemonSetName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid DaemonSet name")
    }

    pub fn deployment_name(&self) -> DeploymentName {
        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= DeploymentName::MAX_LENGTH,
            "The string `<qualified_role_group_name>` must not exceed the limit of Deployment \
            names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_LABEL_NAME;

        DeploymentName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid Deployment name")
    }

    pub fn stateful_set_name(&self) -> StatefulSetName {
        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH <= StatefulSetName::MAX_LENGTH,
            "The string `<qualified_role_group_name>` must not exceed the limit of StatefulSet \
            names."
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
            "The string `<qualified_role_group_name>-headless` must not exceed the limit of \
            Service names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1035_LABEL_NAME;
        let _ = QualifiedRoleGroupName::IS_VALID_LABEL_VALUE;

        ServiceName::from_str(&format!("{}{SUFFIX}", self.qualified_role_group_name()))
            .expect("should be a valid Service name")
    }

    pub fn metrics_service_name(&self) -> ServiceName {
        const SUFFIX: &str = "-metrics";

        // compile-time checks
        const _: () = assert!(
            QualifiedRoleGroupName::MAX_LENGTH + SUFFIX.len() <= ServiceName::MAX_LENGTH,
            "The string `<cluster_name>-<role_name>-<role_group_name>-metrics` must not exceed the \
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
            "The string `<qualified_role_group_name>` must not exceed the limit of Listener names."
        );
        let _ = QualifiedRoleGroupName::IS_RFC_1123_SUBDOMAIN_NAME;

        ListenerName::from_str(self.qualified_role_group_name().as_ref())
            .expect("should be a valid Listener name")
    }
}

#[cfg(test)]
mod tests {
    use super::{ClusterName, RoleGroupName, RoleName};
    use crate::v2::{
        role_group_utils::{QualifiedRoleGroupName, ResourceNames},
        types::kubernetes::{
            ConfigMapName, DaemonSetName, DeploymentName, ListenerName, ServiceName,
            StatefulSetName,
        },
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
            DaemonSetName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.daemon_set_name()
        );
        assert_eq!(
            DeploymentName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.deployment_name()
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
            ServiceName::from_str_unsafe("test-cluster-data-nodes-ssd-storage-metrics"),
            resource_names.metrics_service_name()
        );
        assert_eq!(
            ListenerName::from_str_unsafe("test-cluster-data-nodes-ssd-storage"),
            resource_names.listener_name()
        );
    }

    #[test]
    fn test_fitting_qualified_role_group_name() {
        let cluster_name_length = ClusterName::MAX_LENGTH;
        let role_name_and_role_group_name_length = QualifiedRoleGroupName::MAX_LENGTH - cluster_name_length - 2 /* dashes */;
        let role_name_length = role_name_and_role_group_name_length / 2;
        let role_group_name_length = role_name_and_role_group_name_length - role_name_length;

        let resource_names = ResourceNames {
            cluster_name: ClusterName::from_str_unsafe(&"c".repeat(cluster_name_length)),
            role_name: RoleName::from_str_unsafe(&"r".repeat(role_name_length)),
            role_group_name: RoleGroupName::from_str_unsafe(&"g".repeat(role_group_name_length)),
        };

        let qualified_role_group_name = resource_names.qualified_role_group_name();

        assert_eq!(
            QualifiedRoleGroupName::MAX_LENGTH,
            qualified_role_group_name.to_string().len()
        );
        assert_eq!(
            QualifiedRoleGroupName::from_str_unsafe(
                "cccccccccccccccccccccccccccccccccccccccc-rrrrr-ggggg"
            ),
            qualified_role_group_name
        );
    }

    #[test]
    fn test_hashed_qualified_role_group_name() {
        let resource_names = ResourceNames {
            cluster_name: ClusterName::from_str_unsafe(&"c".repeat(ClusterName::MAX_LENGTH)),
            role_name: RoleName::from_str_unsafe(&"r".repeat(RoleName::MAX_LENGTH)),
            role_group_name: RoleGroupName::from_str_unsafe(&"g".repeat(RoleGroupName::MAX_LENGTH)),
        };

        let qualified_role_group_name = resource_names.qualified_role_group_name();

        assert_eq!(
            QualifiedRoleGroupName::MAX_LENGTH,
            qualified_role_group_name.to_string().len()
        );
        assert_eq!(
            QualifiedRoleGroupName::from_str_unsafe(
                "cccccccccccccccccccccccccccccccccccccccc-rrrr-a12cc0"
            ),
            qualified_role_group_name
        );
    }

    #[test]
    fn test_ensure_max_length() {
        // empty resource name, no hash length
        assert_eq!(
            String::new(),
            ResourceNames::ensure_max_length(String::new(), 2, 0)
        );

        // resource_name.len() <= max_length
        assert_eq!(
            "abcdef".to_owned(),
            ResourceNames::ensure_max_length("abcdef".to_owned(), 6, 4)
        );

        // hash_length == 0
        assert_eq!(
            "abcdef".to_owned(),
            ResourceNames::ensure_max_length("abcdefg".to_owned(), 6, 0)
        );

        // hash appended with dash
        assert_eq!(
            "a-7d1a".to_owned(),
            ResourceNames::ensure_max_length("abcdefg".to_owned(), 6, 4)
        );

        // hash appended without an extra dash
        assert_eq!(
            "ab-a1b1".to_owned(),
            ResourceNames::ensure_max_length("ab-defgh".to_owned(), 7, 4)
        );

        // hash appended without an extra dash
        // In this case, the result is one character shorter than the maximum length.
        assert_eq!(
            "a-3951".to_owned(),
            ResourceNames::ensure_max_length("a-cdefgh".to_owned(), 7, 4)
        );

        // hash appended without an extra dash
        // The two dashes in the given resource name are intentionally kept.
        assert_eq!(
            "a--f7a0".to_owned(),
            ResourceNames::ensure_max_length("a--defgh".to_owned(), 7, 4)
        );

        // A hash_length longer than the produced hash string may not produce the desired result.
        // Just use sensible values!
        assert_eq!(
            "aaaaaaaaa-d476ce01c3787bcab054a2cf48d6af6dd303a0eb549e21a74125132f79d90c36".to_owned(),
            ResourceNames::ensure_max_length("a".repeat(1011), 1010, 1000)
        );
    }
}
