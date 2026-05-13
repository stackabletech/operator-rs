//! Helper functions which are not tied to a specific controller step

use std::str::FromStr;

use snafu::{OptionExt, ResultExt, Snafu};
use stackable_operator::kube::runtime::reflector::Lookup;
use strum::{EnumDiscriminants, IntoStaticStr};

use crate::framework::types::{
    kubernetes::{NamespaceName, Uid},
    operator::ClusterName,
};

#[derive(Snafu, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Error {
    #[snafu(display("failed to get the cluster name"))]
    GetClusterName {},

    #[snafu(display("failed to get the namespace"))]
    GetNamespace {},

    #[snafu(display("failed to get the UID"))]
    GetUid {},

    #[snafu(display("failed to set the cluster name"))]
    ParseClusterName {
        source: crate::framework::macros::attributed_string_type::Error,
    },

    #[snafu(display("failed to set the namespace"))]
    ParseNamespace {
        source: crate::framework::macros::attributed_string_type::Error,
    },

    #[snafu(display("failed to set the UID"))]
    ParseUid {
        source: crate::framework::macros::attributed_string_type::Error,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// Get the cluster name from the given resource
pub fn get_cluster_name(cluster: &impl Lookup) -> Result<ClusterName> {
    let raw_cluster_name = cluster.name().context(GetClusterNameSnafu)?;
    let cluster_name = ClusterName::from_str(&raw_cluster_name).context(ParseClusterNameSnafu)?;

    Ok(cluster_name)
}

/// Get the namespace from the given resource
pub fn get_namespace(resource: &impl Lookup) -> Result<NamespaceName> {
    let raw_namespace = resource.namespace().context(GetNamespaceSnafu)?;
    let namespace = NamespaceName::from_str(&raw_namespace).context(ParseNamespaceSnafu)?;

    Ok(namespace)
}

/// Get the UID from the given resource
pub fn get_uid(resource: &impl Lookup) -> Result<Uid> {
    let raw_uid = resource.uid().context(GetUidSnafu)?;
    let uid = Uid::from_str(&raw_uid).context(ParseUidSnafu)?;

    Ok(uid)
}

#[cfg(test)]
mod tests {
    use stackable_operator::kube::runtime::reflector::Lookup;
    use uuid::uuid;

    use super::{ErrorDiscriminants, get_cluster_name, get_namespace, get_uid};
    use crate::framework::types::{
        kubernetes::{NamespaceName, Uid},
        operator::ClusterName,
    };

    #[derive(Debug, Default)]
    struct TestResource {
        name: Option<&'static str>,
        namespace: Option<&'static str>,
        uid: Option<&'static str>,
    }

    impl Lookup for TestResource {
        type DynamicType = ();

        fn kind(_dyntype: &Self::DynamicType) -> std::borrow::Cow<'_, str> {
            "TestResource".into()
        }

        fn group(_dyntype: &Self::DynamicType) -> std::borrow::Cow<'_, str> {
            "stackable.tech".into()
        }

        fn version(_dyntype: &Self::DynamicType) -> std::borrow::Cow<'_, str> {
            "v1".into()
        }

        fn plural(_dyntype: &Self::DynamicType) -> std::borrow::Cow<'_, str> {
            "testresources".into()
        }

        fn name(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.name.map(std::borrow::Cow::Borrowed)
        }

        fn namespace(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.namespace.map(std::borrow::Cow::Borrowed)
        }

        fn resource_version(&self) -> Option<std::borrow::Cow<'_, str>> {
            Some("1".into())
        }

        fn uid(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.uid.map(std::borrow::Cow::Borrowed)
        }
    }

    #[test]
    fn test_get_cluster_name() {
        assert_eq!(
            ClusterName::from_str_unsafe("test-cluster"),
            get_cluster_name(&TestResource {
                name: Some("test-cluster"),
                ..TestResource::default()
            })
            .expect("should contain a valid cluster name")
        );

        assert_eq!(
            Err(ErrorDiscriminants::GetClusterName),
            get_cluster_name(&TestResource {
                name: None,
                ..TestResource::default()
            })
            .map_err(ErrorDiscriminants::from)
        );

        assert_eq!(
            Err(ErrorDiscriminants::ParseClusterName),
            get_cluster_name(&TestResource {
                name: Some("invalid cluster name"),
                ..TestResource::default()
            })
            .map_err(ErrorDiscriminants::from)
        );
    }

    #[test]
    fn test_get_namespace() {
        assert_eq!(
            NamespaceName::from_str_unsafe("test-namespace"),
            get_namespace(&TestResource {
                namespace: Some("test-namespace"),
                ..TestResource::default()
            })
            .expect("should contain a valid namespace")
        );

        assert_eq!(
            Err(ErrorDiscriminants::GetNamespace),
            get_namespace(&TestResource {
                namespace: None,
                ..TestResource::default()
            })
            .map_err(ErrorDiscriminants::from)
        );

        assert_eq!(
            Err(ErrorDiscriminants::ParseNamespace),
            get_namespace(&TestResource {
                namespace: Some("invalid namespace"),
                ..TestResource::default()
            })
            .map_err(ErrorDiscriminants::from)
        );
    }

    #[test]
    fn test_get_uid() {
        assert_eq!(
            Uid::from(uuid!("e6ac237d-a6d4-43a1-8135-f36506110912")),
            get_uid(&TestResource {
                uid: Some("e6ac237d-a6d4-43a1-8135-f36506110912"),
                ..TestResource::default()
            })
            .expect("should contain a valid UID")
        );

        assert_eq!(
            Err(ErrorDiscriminants::GetUid),
            get_uid(&TestResource {
                uid: None,
                ..TestResource::default()
            })
            .map_err(ErrorDiscriminants::from)
        );

        assert_eq!(
            Err(ErrorDiscriminants::ParseUid),
            get_uid(&TestResource {
                uid: Some("invalid UID"),
                ..TestResource::default()
            })
            .map_err(ErrorDiscriminants::from)
        );
    }
}
