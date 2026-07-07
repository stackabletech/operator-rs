//! Names for operators

use crate::attributed_string_type;

attributed_string_type! {
    ProductName,
    "The name of a product",
    "opensearch",
    // A suffix is added to produce a label value. An according compile-time check ensures that
    // max_length cannot be set higher.
    (max_length = 54),
    is_rfc_1123_dns_subdomain_name,
    is_valid_label_value
}

attributed_string_type! {
    ProductVersion,
    "The version of a product",
    "3.4.0",
    is_valid_label_value
}

attributed_string_type! {
    ClusterName,
    "The name of a cluster/stacklet",
    "my-opensearch-cluster",
    // Suffixes are added to produce resource names.
    //
    // 40 characters for cluster names should be sufficient and still allow the operators to append
    // custom suffixes to build resource names. Increasing this value could break existing operator
    // code.
    (max_length = 40),
    is_rfc_1035_label_name,
    is_valid_label_value
}

attributed_string_type! {
    ControllerName,
    "The name of a controller in an operator",
    "opensearchcluster",
    is_valid_label_value
}

attributed_string_type! {
    OperatorName,
    "The name of an operator",
    "opensearch.stackable.tech",
    is_valid_label_value
}

attributed_string_type! {
    RoleGroupName,
    "The name of a role-group name",
    "cluster-manager",
    is_rfc_1123_label_name,
    is_valid_label_value
}

attributed_string_type! {
    RoleName,
    "The name of a role name",
    "nodes",
    is_rfc_1123_label_name,
    is_valid_label_value
}

#[cfg(test)]
mod tests {
    use super::{
        ClusterName, ControllerName, OperatorName, ProductName, ProductVersion, RoleGroupName,
        RoleName,
    };

    #[test]
    fn test_attributed_string_type_examples() {
        ProductName::test_example();
        ProductVersion::test_example();
        ClusterName::test_example();
        ControllerName::test_example();
        OperatorName::test_example();
        RoleGroupName::test_example();
        RoleName::test_example();
    }
}
