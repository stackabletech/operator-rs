//! The "Stackable" way to implement our Kubernetes resource names.
//!
//! We follow the specification for RFC 1123 Names (https://tools.ietf.org/html/rfc1123):
//! This means the name must:
//! * contain at most 63 characters
//! * contain only lowercase alphanumeric characters or '-'
//! * start with an alphanumeric character
//! * end with an alphanumeric character
//!
//! The Stackable name is structured in the following pattern ("[min..max]" indicates the
//! amount of possible characters. Everything below throws an error and everything above is cut off
//! and "(<...>)" is optional:
//! <short-name[1..5]>-<cluster-name[1..10]>-<role_name[1..10]>-<group_name[1..8]>-(<node_name[0..10]>)-(<misc[0..5]>)-
//!
//! Additionally all non alphanumeric characters are removed.
//!
//! This generated name is supposed to work with the generatedName (to add a unique hash) from Kubernetes.
//!
use crate::error::{Error, OperatorResult};
use strum_macros::EnumIter;

const SHORT_NAME_MIN_LEN: u8 = 1;
const SHORT_NAME_MAX_LEN: u8 = 5;
const CLUSTER_NAME_MIN_LEN: u8 = 1;
const CLUSTER_NAME_MAX_LEN: u8 = 10;
const ROLE_NAME_MIN_LEN: u8 = 1;
const ROLE_NAME_MAX_LEN: u8 = 10;
const GROUP_NAME_MIN_LEN: u8 = 1;
const GROUP_NAME_MAX_LEN: u8 = 8;
const NODE_NAME_MIN_LEN: u8 = 0;
const NODE_NAME_MAX_LEN: u8 = 10;
const MISC_MIN_LEN: u8 = 0;
const MISC_MAX_LEN: u8 = 5;

/// The sub names that make up the full resource name.
#[derive(Debug, strum_macros::Display, strum_macros::EnumString, EnumIter)]
enum SubName {
    /// CustomResourceDefinition short name
    Short,
    /// The CustomResource name
    Cluster,
    /// The CustomResource role name
    Role,
    /// The CustomResource role group name
    Group,
    /// The optional node name (e.g. for pods)
    Node,
    /// Miscellaneous identifiers (e.g. "data" or "conf" for config maps)
    Misc,
}

impl SubName {
    /// Returns the minimum length for each sub name
    fn min(&self) -> u8 {
        match self {
            SubName::Short => SHORT_NAME_MIN_LEN,
            SubName::Cluster => CLUSTER_NAME_MIN_LEN,
            SubName::Role => ROLE_NAME_MIN_LEN,
            SubName::Group => GROUP_NAME_MIN_LEN,
            SubName::Node => NODE_NAME_MIN_LEN,
            SubName::Misc => MISC_MIN_LEN,
        }
    }

    /// Returns the maximum length for each sub name
    fn max(&self) -> u8 {
        match self {
            SubName::Short => SHORT_NAME_MAX_LEN,
            SubName::Cluster => CLUSTER_NAME_MAX_LEN,
            SubName::Role => ROLE_NAME_MAX_LEN,
            SubName::Group => GROUP_NAME_MAX_LEN,
            SubName::Node => NODE_NAME_MAX_LEN,
            SubName::Misc => MISC_MAX_LEN,
        }
    }
}

/// Build a Kubernetes resource name. This is intended to work with the generatedName (add a unique
/// hash) from Kubernetes. This method ensures that all single components do not exceed a certain
/// length in order to keep the resource name below 63 (the maximum allowed characters) minus an
/// offset for the unique hash.
///
/// In each sub name (`short_name`, `cluster_name`...) all non alphanumeric parts are automatically
/// removed. After the removal each sub_name is cut off if it exceeds its specified length.
///
/// After processing each sub name is concatenated with "-" and "-" as the last character (to
/// separate from the kubernetes hash).
///
/// # Arguments
///
/// * `short_name` - The short name of the custom resource definition.
/// * `cluster_name` - The name of the custom resource.
/// * `role_name` - The role name of the custom resource.
/// * `group_name` - The group name of the custom resource.
/// * `node_name` - Optional node name if available (e.g. for pods).
/// * `misc_name` - Optional miscellaneous identifiers (e.g. "data" for config maps).
///
pub fn build_resource_name(
    short_name: &str,
    cluster_name: &str,
    role_name: &str,
    group_name: &str,
    node_name: Option<&str>,
    misc_name: Option<&str>,
) -> OperatorResult<String> {
    let mut full_name = String::new();

    full_name.push_str(&strip(SubName::Short, short_name)?);
    full_name.push('-');

    full_name.push_str(&strip(SubName::Cluster, cluster_name)?);
    full_name.push('-');

    full_name.push_str(&strip(SubName::Role, role_name)?);
    full_name.push('-');

    full_name.push_str(&strip(SubName::Group, group_name)?);
    full_name.push('-');

    if let Some(node) = node_name {
        full_name.push_str(&strip(SubName::Node, node)?);
        full_name.push('-');
    };

    if let Some(misc) = misc_name {
        full_name.push_str(&strip(SubName::Misc, misc)?);
        full_name.push('-');
    };

    Ok(full_name.to_lowercase())
}

/// This method removes all non alphanumeric characters from a sub name, checks if the
/// length after the removal is not below the minimum (throws error) or over the maximum
/// (will be cut off) specified length.
///
/// # Arguments
///
/// * `kind` - The kind of the sub_name (e.g. ShortName, ClusterName...).
/// * `sub_name` - The sub_name to be processed.
///
fn strip(kind: SubName, sub_name: &str) -> OperatorResult<String> {
    let alphanumeric: String = sub_name.chars().filter(|c| c.is_alphanumeric()).collect();

    let min = kind.min();
    if alphanumeric.len() < usize::from(min) {
        return Err(Error::SubNameTooShort {
            kind: kind.to_string(),
            name: sub_name.to_string(),
            min,
        });
    }

    Ok(alphanumeric
        .chars()
        .into_iter()
        .take(usize::from(kind.max()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use strum::IntoEnumIterator;

    const GENERATED_HASH_MIN_LENGTH: u8 = 8;
    const FULL_NAME_MAX_LEN: u8 = 63;

    #[test]
    fn test_name_max_len() {
        let mut length = 0;
        for name in SubName::iter() {
            length += name.max();
        }

        assert!(length <= FULL_NAME_MAX_LEN - GENERATED_HASH_MIN_LENGTH);
    }

    #[rstest]
    #[case(SubName::Short, "long_short_name", "longs")]
    #[case(SubName::Cluster, "pr!od.text1-1234a-asdcv", "prodtext11")]
    #[case(SubName::Misc, "", "")]
    fn test_strip_ok(#[case] kind: SubName, #[case] name: &str, #[case] expected: &str) {
        let result = strip(kind, name).unwrap();
        assert_eq!(&result, expected);
    }

    #[rstest]
    #[case(SubName::Cluster, "")]
    fn test_strip_err(#[case] kind: SubName, #[case] name: &str) {
        assert!(strip(kind, name).is_err());
    }

    #[rstest]
    #[case(
        "zk",
        "prod",
        "server",
        "default",
        None,
        None,
        "zk-prod-server-default-"
    )]
    #[case(
        "zookeeper",
        "production",
        "server",
        "default",
        Some("aws.test-server-cluster.123456789"),
        None,
        "zooke-production-server-default-awstestser-"
    )]
    #[case(
        "zookeeper",
        "production.hamburg",
        "server!&_big_cloud",
        "default",
        None,
        None,
        "zooke-production-serverbigc-default-"
    )]
    #[case(
        ".-zookeeper",
        "production.hamburg",
        "server!&_big_cloud",
        "default",
        Some("aws.test-server-cluster.123456789"),
        Some("config"),
        "zooke-production-serverbigc-default-awstestser-confi-"
    )]
    fn test_build_resource_name_ok(
        #[case] short_name: &str,
        #[case] cluster_name: &str,
        #[case] role_name: &str,
        #[case] group_name: &str,
        #[case] node_name: Option<&str>,
        #[case] misc_name: Option<&str>,
        #[case] expected: &str,
    ) {
        let resource_name = &build_resource_name(
            short_name,
            cluster_name,
            role_name,
            group_name,
            node_name,
            misc_name,
        )
        .unwrap();
        assert_eq!(resource_name, expected);
        assert!(resource_name.len() <= usize::from(FULL_NAME_MAX_LEN - GENERATED_HASH_MIN_LENGTH));
    }
}
