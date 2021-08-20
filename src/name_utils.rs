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

#[derive(Debug, strum_macros::Display, strum_macros::EnumString, EnumIter)]
enum SubName {
    Short,
    Cluster,
    Role,
    Group,
    Node,
    Misc,
}

impl SubName {
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
