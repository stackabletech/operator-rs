//! Kubernetes (resource) names
use std::str::FromStr;

use stackable_operator::validation::{RFC_1123_LABEL_MAX_LENGTH, RFC_1123_SUBDOMAIN_MAX_LENGTH};

use crate::attributed_string_type;

attributed_string_type! {
    ConfigMapName,
    "The name of a ConfigMap",
    "opensearch-nodes-default",
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    ConfigMapKey,
    "The key for a ConfigMap",
    "log4j2.properties",
    (min_length = 1),
    // see https://github.com/kubernetes/kubernetes/blob/v1.34.1/staging/src/k8s.io/apimachinery/pkg/util/validation/validation.go#L435-L451
    (max_length = RFC_1123_SUBDOMAIN_MAX_LENGTH),
    (regex = "^[-._a-zA-Z0-9]+$")
}

attributed_string_type! {
    ContainerName,
    "The name of a container in a Pod",
    "opensearch",
    is_rfc_1123_label_name
}

attributed_string_type! {
    ClusterRoleName,
    "The name of a ClusterRole",
    "opensearch-clusterrole",
    // On the one hand, ClusterRoles must only contain characters that are allowed for DNS
    // subdomain names, on the other hand, their length does not seem to be restricted – at least
    // on Kind. However, 253 characters are sufficient for the Stackable operators, and to avoid
    // problems on other Kubernetes providers, the length is restricted here.
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    Hostname,
    "A hostname",
    "example.com",
    (min_length = 1),
    (max_length = 253),
    // see https://en.wikipedia.org/wiki/Hostname#Syntax
    (regex = "^[a-zA-Z0-9]([-a-zA-Z0-9]{0,60}[a-zA-Z0-9])?(\\.[a-zA-Z0-9]([-a-zA-Z0-9]{0,60}[a-zA-Z0-9])?)*\\.?$")
}

attributed_string_type! {
    ListenerName,
    "The name of a Listener",
    "opensearch-nodes-default",
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    ListenerClassName,
    "The name of a Listener",
    "external-stable",
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    NamespaceName,
    "The name of a Namespace",
    "stackable-operators",
    is_rfc_1123_label_name,
    is_valid_label_value
}

attributed_string_type! {
    PersistentVolumeClaimName,
    "The name of a PersistentVolumeClaim",
    "config",
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    RoleBindingName,
    "The name of a RoleBinding",
    "opensearch-rolebinding",
    // On the one hand, RoleBindings must only contain characters that are allowed for DNS
    // subdomain names, on the other hand, their length does not seem to be restricted – at least
    // on Kind. However, 253 characters are sufficient for the Stackable operators, and to avoid
    // problems on other Kubernetes providers, the length is restricted here.
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    SecretClassName,
    "The name of a SecretClass",
    "tls",
    // The secret class name is used in an annotation on the tls volume.
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    SecretKey,
    "The key for a Secret",
    "accessKey",
    (min_length = 1),
    // see https://github.com/kubernetes/kubernetes/blob/v1.34.1/staging/src/k8s.io/apimachinery/pkg/util/validation/validation.go#L435-L451
    (max_length = RFC_1123_SUBDOMAIN_MAX_LENGTH),
    (regex = "^[-._a-zA-Z0-9]+$")
}

attributed_string_type! {
    SecretName,
    "The name of a Secret",
    "opensearch-security-config",
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    ServiceAccountName,
    "The name of a ServiceAccount",
    "opensearch-serviceaccount",
    is_rfc_1123_dns_subdomain_name
}

attributed_string_type! {
    ServiceName,
    "The name of a Service",
    "opensearch-nodes-default-headless",
    is_rfc_1035_label_name,
    is_valid_label_value
}

attributed_string_type! {
    StatefulSetName,
    "The name of a StatefulSet",
    "opensearch-nodes-default",
    (max_length =
        // see https://github.com/kubernetes/kubernetes/issues/64023
        RFC_1123_LABEL_MAX_LENGTH
            - 1 /* dash */
            - 10 /* digits for the controller-revision-hash label */),
    is_rfc_1123_label_name,
    is_valid_label_value
}

attributed_string_type! {
    Uid,
    "A UID",
    "c27b3971-ca72-42c1-80a4-abdfc1db0ddd",
    is_uid,
    is_valid_label_value
}

attributed_string_type! {
    VolumeName,
    "The name of a Volume",
    "opensearch-nodes-default",
    is_rfc_1123_label_name,
    is_valid_label_value
}

#[cfg(test)]
mod tests {
    use super::{
        ClusterRoleName, ConfigMapKey, ConfigMapName, ContainerName, Hostname, ListenerClassName,
        ListenerName, NamespaceName, PersistentVolumeClaimName, RoleBindingName, SecretClassName,
        SecretKey, SecretName, ServiceAccountName, ServiceName, StatefulSetName, Uid, VolumeName,
    };

    #[test]
    fn test_attributed_string_type_examples() {
        ConfigMapName::test_example();
        ConfigMapKey::test_example();
        ContainerName::test_example();
        ClusterRoleName::test_example();
        Hostname::test_example();
        ListenerName::test_example();
        ListenerClassName::test_example();
        NamespaceName::test_example();
        PersistentVolumeClaimName::test_example();
        RoleBindingName::test_example();
        SecretClassName::test_example();
        SecretKey::test_example();
        SecretName::test_example();
        ServiceAccountName::test_example();
        ServiceName::test_example();
        StatefulSetName::test_example();
        Uid::test_example();
        VolumeName::test_example();
    }
}
