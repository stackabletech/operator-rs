use crate::{
    builder::meta::ObjectMetaBuilder,
    k8s_openapi::{
        Resource as KubeApiResource,
        api::{
            core::v1::ServiceAccount,
            rbac::v1::{ClusterRole, RoleBinding, RoleRef, Subject},
        },
        apimachinery::pkg::apis::meta::v1::ObjectMeta,
    },
    kube::{Resource, ResourceExt},
    kvp::Labels,
    v2::{HasName, HasUid, builder::meta::ownerreference_from_resource, role_utils::ResourceNames},
};

/// Builds the [`ServiceAccount`] for the product workloads, named
/// `<cluster_name>-serviceaccount` after the given [`ResourceNames`].
///
/// Together with [`build_role_binding`] this is the infallible variant of
/// [`crate::commons::rbac::build_rbac_resources`]; the output is identical as long as
/// `resource_names.cluster_name` matches the metadata name of `owner`.
pub fn build_service_account(
    owner: &(impl Resource<DynamicType = ()> + HasName + HasUid),
    resource_names: &ResourceNames,
    labels: Labels,
) -> ServiceAccount {
    ServiceAccount {
        metadata: build_metadata(
            owner,
            resource_names.service_account_name().to_string(),
            labels,
        ),
        ..ServiceAccount::default()
    }
}

/// Builds the [`RoleBinding`] for the product workloads, named `<cluster_name>-rolebinding`
/// after the given [`ResourceNames`]. It binds the [`ServiceAccount`] from
/// [`build_service_account`] to the operator-deployed ClusterRole
/// `<product_name>-clusterrole`, which must already exist.
///
/// Together with [`build_service_account`] this is the infallible variant of
/// [`crate::commons::rbac::build_rbac_resources`]; the output is identical as long as
/// `resource_names.cluster_name` matches the metadata name of `owner`.
pub fn build_role_binding(
    owner: &(impl Resource<DynamicType = ()> + HasName + HasUid),
    resource_names: &ResourceNames,
    labels: Labels,
) -> RoleBinding {
    RoleBinding {
        metadata: build_metadata(
            owner,
            resource_names.role_binding_name().to_string(),
            labels,
        ),
        role_ref: RoleRef {
            api_group: Some(ClusterRole::GROUP.to_owned()),
            kind: ClusterRole::KIND.to_owned(),
            name: resource_names.cluster_role_name().to_string(),
        },
        subjects: Some(vec![Subject {
            kind: ServiceAccount::KIND.to_owned(),
            name: resource_names.service_account_name().to_string(),
            namespace: owner.namespace(),
            // Left unset because the ServiceAccount kind is in the core API group.
            api_group: None,
        }]),
    }
}

/// Common metadata of the RBAC resources: name, the owner's namespace, an owner reference on
/// `owner` and the given labels.
fn build_metadata(
    owner: &(impl Resource<DynamicType = ()> + HasName + HasUid),
    name: impl Into<String>,
    labels: Labels,
) -> ObjectMeta {
    ObjectMetaBuilder::new()
        .name(name)
        .namespace_opt(owner.namespace())
        .ownerreference(ownerreference_from_resource(owner, None, Some(true)))
        .with_labels(labels)
        .build()
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::{
        k8s_openapi::{
            api::{
                core::v1::ServiceAccount,
                rbac::v1::{RoleBinding, RoleRef, Subject},
            },
            apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference},
        },
        kube::Resource,
        kvp::Labels,
        v2::{
            HasName, HasUid,
            rbac::{build_role_binding, build_service_account},
            role_utils::ResourceNames,
            types::{
                kubernetes::Uid,
                operator::{ClusterName, ProductName},
            },
        },
    };

    #[derive(Clone)]
    struct Cluster {
        object_meta: ObjectMeta,
    }

    impl Cluster {
        fn new() -> Self {
            Self {
                object_meta: ObjectMeta {
                    name: Some("cluster-name".to_owned()),
                    namespace: Some("cluster-namespace".to_owned()),
                    uid: Some("a6b89911-d48e-4328-88d6-b9251226583d".to_owned()),
                    ..ObjectMeta::default()
                },
            }
        }
    }

    impl Resource for Cluster {
        type DynamicType = ();
        type Scope = ();

        fn kind(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("kind")
        }

        fn group(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("group")
        }

        fn version(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("version")
        }

        fn plural(_dt: &Self::DynamicType) -> Cow<'_, str> {
            Cow::from("plural")
        }

        fn meta(&self) -> &ObjectMeta {
            &self.object_meta
        }

        fn meta_mut(&mut self) -> &mut ObjectMeta {
            &mut self.object_meta
        }
    }

    impl HasName for Cluster {
        fn to_name(&self) -> String {
            self.object_meta
                .name
                .clone()
                .expect("should be set in Cluster::new")
        }
    }

    impl HasUid for Cluster {
        fn to_uid(&self) -> Uid {
            Uid::from_str_unsafe(
                &self
                    .object_meta
                    .uid
                    .clone()
                    .expect("should be set in Cluster::new"),
            )
        }
    }

    fn resource_names() -> ResourceNames {
        ResourceNames {
            cluster_name: ClusterName::from_str_unsafe("cluster-name"),
            product_name: ProductName::from_str_unsafe("my-product"),
        }
    }

    fn labels() -> Labels {
        Labels::common("my-product", "cluster-name").expect("should be valid label values")
    }

    fn expected_metadata(name: &str) -> ObjectMeta {
        ObjectMeta {
            labels: Some(
                [
                    ("app.kubernetes.io/instance", "cluster-name"),
                    ("app.kubernetes.io/name", "my-product"),
                ]
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .into(),
            ),
            name: Some(name.to_owned()),
            namespace: Some("cluster-namespace".to_owned()),
            owner_references: Some(vec![OwnerReference {
                api_version: "group/version".to_owned(),
                controller: Some(true),
                kind: "kind".to_owned(),
                name: "cluster-name".to_owned(),
                uid: "a6b89911-d48e-4328-88d6-b9251226583d".to_owned(),
                ..OwnerReference::default()
            }]),
            ..ObjectMeta::default()
        }
    }

    #[test]
    fn build_expected_service_account() {
        let expected_service_account = ServiceAccount {
            metadata: expected_metadata("cluster-name-serviceaccount"),
            ..ServiceAccount::default()
        };

        assert_eq!(
            expected_service_account,
            build_service_account(&Cluster::new(), &resource_names(), labels())
        );
    }

    #[test]
    fn build_expected_role_binding() {
        let expected_role_binding = RoleBinding {
            metadata: expected_metadata("cluster-name-rolebinding"),
            role_ref: RoleRef {
                api_group: Some("rbac.authorization.k8s.io".to_owned()),
                kind: "ClusterRole".to_owned(),
                name: "my-product-clusterrole".to_owned(),
            },
            subjects: Some(vec![Subject {
                kind: "ServiceAccount".to_owned(),
                name: "cluster-name-serviceaccount".to_owned(),
                namespace: Some("cluster-namespace".to_owned()),
                api_group: None,
            }]),
        };

        assert_eq!(
            expected_role_binding,
            build_role_binding(&Cluster::new(), &resource_names(), labels())
        );
    }

    #[test]
    fn output_is_identical_to_v1_build_rbac_resources() {
        let cluster = Cluster::new();

        let (v1_service_account, v1_role_binding) =
            crate::commons::rbac::build_rbac_resources(&cluster, "my-product", labels())
                .expect("should build the v1 RBAC resources");

        assert_eq!(
            v1_service_account,
            build_service_account(&cluster, &resource_names(), labels())
        );
        assert_eq!(
            v1_role_binding,
            build_role_binding(&cluster, &resource_names(), labels())
        );
    }
}
