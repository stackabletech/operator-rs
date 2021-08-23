//! The "Stackable" way to implement our Kubernetes resource names.
//!
//! We follow the specification for RFC 1035 names (https://tools.ietf.org/html/rfc1035):
//! This means the name must:
//! * contain at most 63 characters
//! * contain only lowercase alphanumeric characters or '-'
//! * start with an alphabetic character
//! * end with an alphanumeric character
//!
//! The Stackable name is structured in the following pattern ("(<...>)" means optional):
//! <short-name>-<cluster-name>-<role_name>-(<group_name>)-(<node_name>)-(<misc>)-
//!
//! Additionally all non alphanumeric characters are removed.
//!
//! This generated name is supposed to work with the generatedName (to add a unique hash) from Kubernetes.
//!
use crate::error::OperatorResult;

/// Adjustable number of characters reserved for a generated Kubernetes hash.
const KUBERNETES_HASH_MIN_LENGTH: usize = 8;
/// This is the overall fixed length for resource names.
/// WARNING: Do not change this number unless any specifications change.
const RESOURCE_NAME_MAX_LEN: usize = 63;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The provided sub name for [{kind}] is empty. This is not allowed.")]
    SubNameEmpty { kind: String },

    #[error(
        "The provided sub name for [{kind}] does not start with an alphanumeric character [{name}]."
    )]
    SubNameDoesNotStartAlphabetic { kind: String, name: String },
}

/// The sub names that make up the full resource name.
#[derive(Debug, PartialEq, PartialOrd, strum_macros::Display)]
enum SubName {
    /// CustomResourceDefinition short name
    #[strum(serialize = "ShortName")]
    Short,
    /// The CustomResource name
    #[strum(serialize = "ClusterName")]
    Cluster,
    /// The CustomResource role name
    #[strum(serialize = "RoleName")]
    Role,
    /// The CustomResource role group name
    #[strum(serialize = "GroupName")]
    Group,
    /// The optional node name (e.g. for pods)
    #[strum(serialize = "NodeName")]
    Node,
    /// Miscellaneous identifiers (e.g. "data" or "conf" for config maps)
    #[strum(serialize = "MiscName")]
    Misc,
}

/// Build a Kubernetes resource name. This is intended to work with the generatedName (add a unique
/// hash) from Kubernetes. This method ensures that all single components do not exceed a certain
/// length in order to keep the resource name below or equal to 63 (the maximum allowed characters)
/// minus an offset for the unique hash (`KUBERNETES_HASH_MIN_LENGTH`).
///
/// In each sub name (`short_name`, `cluster_name`...) all non alphanumeric parts are automatically
/// removed. After processing, each sub name is concatenated with "-" and added "-" as the last
/// character (to separate from the kubernetes hash).
///
/// The basic sub name length for each sub name is calculated as follows:
/// (1 / number_of_sub_names) * (RESOURCE_NAME_MAX_LEN - KUBERNETES_HASH_MIN_LENGTH - number_of_sub_names)
///
/// If sub names are shorter than the basic length, these characters can be used by another (longer)
/// sub name.
///
/// # Arguments
///
/// * `short_name` - The short name of the custom resource definition.
/// * `cluster_name` - The name of the custom resource.
/// * `role_name` - The role name of the custom resource.
/// * `group_name` - Optional group name of the custom resource.
/// * `node_name` - Optional node name if available (e.g. for pods).
/// * `misc_name` - Optional miscellaneous identifiers (e.g. "data" for config maps).
///
pub fn build_resource_name(
    short_name: &str,
    cluster_name: &str,
    role_name: &str,
    group_name: Option<&str>,
    node_name: Option<&str>,
    misc_name: Option<&str>,
) -> OperatorResult<String> {
    // collect a vector of all sub names and their respective kind
    let mut sub_names = vec![
        // transform to alphanumeric and lowercase
        to_alphanumeric_lowercase_not_empty(SubName::Short, short_name)?,
        to_alphanumeric_lowercase_not_empty(SubName::Cluster, cluster_name)?,
        to_alphanumeric_lowercase_not_empty(SubName::Role, role_name)?,
    ];

    if let Some(group) = group_name {
        sub_names.push(to_alphanumeric_lowercase_not_empty(SubName::Group, group)?);
    }

    if let Some(node) = node_name {
        sub_names.push(to_alphanumeric_lowercase_not_empty(SubName::Node, node)?);
    };

    if let Some(misc) = misc_name {
        sub_names.push(to_alphanumeric_lowercase_not_empty(SubName::Misc, misc)?);
    };

    // Calculate the maximum number of available characters
    // The sub names vector length equals the size of additional required dashes
    let max_chars = RESOURCE_NAME_MAX_LEN - KUBERNETES_HASH_MIN_LENGTH - sub_names.len();
    // The amount of characters receives without carryover (even distributed)
    let selectable_chars = selectable_chars(&sub_names, max_chars);
    // The `carryover` are left over characters that will not be used by some sub names
    // and can be added to other longer sub names exceeding the amount of `selectable_chars`.
    let carryover = max_chars - used_characters(&sub_names, selectable_chars);

    build_name(sub_names, selectable_chars, carryover)
}

/// This splits the number of available characters into equal blocks for each sub name and
/// returns its size.
///
/// # Arguments
///
/// * `sub_names` - A vector of available sub names.
/// * `max_chars` - The maximum available characters:
///                 `RESOURCE_NAME_MAX_LEN` - `KUBERNETES_HASH_MIN_LENGTH` - sub_names.len()
///
fn selectable_chars(sub_names: &[String], max_chars: usize) -> usize {
    (1f32 / sub_names.len() as f32 * max_chars as f32).floor() as usize
}

/// This calculates the sum of used characters from each sub name. Sub names that exceed the
/// amount of `selectable_chars` will count as the number of `selectable_chars`.
///
/// # Arguments
///
/// * `sub_names` - A vector of available sub names.
/// * `selectable_chars` - Even distributed amount of characters that may be used for each sub name.
///
fn used_characters(sub_names: &[String], selectable_chars: usize) -> usize {
    let mut used = 0;

    for name in sub_names {
        if selectable_chars > name.len() {
            used += name.len();
        } else {
            used += selectable_chars;
        }
    }

    used
}

/// This method concatenates each sub name and dynamically adapts the length of each sub name if
/// `unused_chars` are available. The order of 'distributing' the unused_chars is determined by
/// the sub_names vector. Items that come first may receive all `unused_chars` while items in
/// the end have to be cut down to the length of `selectable_chars`.
///
/// # Arguments
///
/// * `sub_names` - A vector of available sub names.
/// * `selectable_chars` - Even distributed amount of characters that may be used for each sub name.
/// * `unused_chars` - Number of characters that are not used by some sub names and may be added to
///                    other longer sub names.
///
fn build_name(
    sub_names: Vec<String>,
    selectable_chars: usize,
    unused_chars: usize,
) -> OperatorResult<String> {
    let mut full_name = String::new();
    let mut carryover = unused_chars;

    for name in sub_names {
        let selected: String = name
            .chars()
            .into_iter()
            .take(selectable_chars + carryover)
            .collect();

        // if the sub name was extended via carryover, we need to adapt how many
        // carryover characters we have left.
        if selected.len() > selectable_chars {
            carryover -= selected.len() - selectable_chars;
        }

        full_name.push_str(&selected);
        full_name.push('-');
    }

    Ok(full_name)
}

/// This will remove all non alphanumeric characters from a sub_name. If the sub name is empty an
/// error is thrown.
/// If the sub name is SubName::Short, it should start with an alphabetic character. If not, an
/// error is thrown.
///
/// # Arguments
///
/// * `kind` - The kind of the sub name (only required for error handling).
/// * `sub_name` - The sub name to process.
///
fn to_alphanumeric_lowercase_not_empty(kind: SubName, sub_name: &str) -> Result<String, Error> {
    if sub_name.is_empty() {
        return Err(Error::SubNameEmpty {
            kind: kind.to_string(),
        });
    }

    if kind == SubName::Short {
        if let Some(c) = sub_name.chars().next() {
            if !c.is_alphabetic() {
                return Err(Error::SubNameDoesNotStartAlphabetic {
                    kind: kind.to_string(),
                    name: sub_name.to_string(),
                });
            }
        }
    }

    Ok(sub_name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    //use rstest::*;

    #[test]
    fn test_abcde() {
        let result = build_resource_name(
            "zkaaaaa11",
            "cluster_name_very_long",
            "role22",
            Some("group222"),
            Some("node123456"),
            Some("conf"),
        )
        .unwrap();
        println!("{} ({})", result, result.len());
    }

    // #[test]
    // fn test_name_max_len() {
    //     let mut length = 0;
    //     for name in SubName::iter() {
    //         length += name.max();
    //     }
    //
    //     assert!(length <= FULL_NAME_MAX_LEN - GENERATED_HASH_MIN_LENGTH);
    // }
    //
    // #[rstest]
    // #[case(SubName::Short, "long_short_name", "longs")]
    // #[case(SubName::Cluster, "pr!od.text1-1234a-asdcv", "prodtext11")]
    // #[case(SubName::Misc, "", "")]
    // fn test_strip_ok(#[case] kind: SubName, #[case] name: &str, #[case] expected: &str) {
    //     let result = strip(kind, name).unwrap();
    //     assert_eq!(&result, expected);
    // }
    //
    // #[rstest]
    // #[case(SubName::Cluster, "")]
    // fn test_strip_err(#[case] kind: SubName, #[case] name: &str) {
    //     assert!(strip(kind, name).is_err());
    // }
    //
    // #[rstest]
    // #[case(
    //     "zk",
    //     "prod",
    //     "server",
    //     Some("default"),
    //     None,
    //     None,
    //     "zk-prod-server-default-"
    // )]
    // #[case(
    //     "zookeeper",
    //     "production",
    //     "server",
    //     Some("default"),
    //     Some("aws.test-server-cluster.123456789"),
    //     None,
    //     "zooke-production-server-default-awstestser-"
    // )]
    // #[case(
    //     "zookeeper",
    //     "production.hamburg",
    //     "server!&_big_cloud",
    //     Some("default"),
    //     None,
    //     None,
    //     "zooke-production-serverbigc-default-"
    // )]
    // #[case(
    //     ".-zookeeper",
    //     "production.hamburg",
    //     "server!&_big_cloud",
    //     Some("default"),
    //     Some("aws.test-server-cluster.123456789"),
    //     Some("config"),
    //     "zooke-production-serverbigc-default-awstestser-confi-"
    // )]
    // fn test_build_resource_name_ok(
    //     #[case] short_name: &str,
    //     #[case] cluster_name: &str,
    //     #[case] role_name: &str,
    //     #[case] group_name: Option<&str>,
    //     #[case] node_name: Option<&str>,
    //     #[case] misc_name: Option<&str>,
    //     #[case] expected: &str,
    // ) {
    //     let resource_name = &build_resource_name(
    //         short_name,
    //         cluster_name,
    //         role_name,
    //         group_name,
    //         node_name,
    //         misc_name,
    //     )
    //     .unwrap();
    //     assert_eq!(resource_name, expected);
    //     assert!(resource_name.len() <= usize::from(FULL_NAME_MAX_LEN - GENERATED_HASH_MIN_LENGTH));
    // }
}
