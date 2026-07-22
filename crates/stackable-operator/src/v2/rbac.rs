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
    v2::{
        HasName, HasUid, NameIsValidLabelValue,
        builder::meta::ownerreference_from_resource,
        controller_utils::ContextNames,
        kvp::label::{NONE_ROLE_GROUP_NAME, NONE_ROLE_NAME},
        role_utils::ClusterResourceNames,
        types::operator::ProductVersion,
    },
};

/// Builds the [`ServiceAccount`] for the product workloads, named
/// `<cluster_name>-serviceaccount` after the given [`ClusterResourceNames`], carrying the
/// recommended labels derived from `context_names` and `product_version` (with
/// [`static@NONE_ROLE_NAME`]/[`static@NONE_ROLE_GROUP_NAME`] as the role/role-group values,
/// because the ServiceAccount is shared by the whole cluster).
///
/// Together with [`build_role_binding`] this is the infallible variant of
/// [`crate::commons::rbac::build_rbac_resources`]; the output is identical when the v1
/// function is given the same recommended labels and `resource_names.cluster_name` matches
/// the metadata name of `owner`.
pub fn build_service_account(
    owner: &(impl Resource<DynamicType = ()> + HasName + HasUid + NameIsValidLabelValue),
    resource_names: &ClusterResourceNames,
    context_names: &ContextNames,
    product_version: &ProductVersion,
) -> ServiceAccount {
    ServiceAccount {
        metadata: build_metadata(
            owner,
            resource_names.service_account_name().to_string(),
            rbac_labels(owner, context_names, product_version),
        ),
        ..ServiceAccount::default()
    }
}

/// Builds the [`RoleBinding`] for the product workloads, named `<cluster_name>-rolebinding`
/// after the given [`ClusterResourceNames`] and labelled like [`build_service_account`]. It
/// binds the [`ServiceAccount`] from [`build_service_account`] to the operator-deployed
/// ClusterRole `<product_name>-clusterrole`, which must already exist.
///
/// Together with [`build_service_account`] this is the infallible variant of
/// [`crate::commons::rbac::build_rbac_resources`]; the output is identical when the v1
/// function is given the same recommended labels and `resource_names.cluster_name` matches
/// the metadata name of `owner`.
pub fn build_role_binding(
    owner: &(impl Resource<DynamicType = ()> + HasName + HasUid + NameIsValidLabelValue),
    resource_names: &ClusterResourceNames,
    context_names: &ContextNames,
    product_version: &ProductVersion,
) -> RoleBinding {
    RoleBinding {
        metadata: build_metadata(
            owner,
            resource_names.role_binding_name().to_string(),
            rbac_labels(owner, context_names, product_version),
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

/// The recommended labels of the RBAC resources; role and role group are
/// [`static@NONE_ROLE_NAME`]/[`static@NONE_ROLE_GROUP_NAME`] because the resources are shared
/// by the whole cluster.
fn rbac_labels(
    owner: &(impl Resource + HasName + NameIsValidLabelValue),
    context_names: &ContextNames,
    product_version: &ProductVersion,
) -> Labels {
    context_names.recommended_labels(
        owner,
        product_version,
        &NONE_ROLE_NAME,
        &NONE_ROLE_GROUP_NAME,
    )
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
        v2::{
            HasName, HasUid, NameIsValidLabelValue,
            controller_utils::ContextNames,
            kvp::label::{NONE_ROLE_GROUP_NAME, NONE_ROLE_NAME, recommended_labels},
            rbac::{build_role_binding, build_service_account},
            role_utils::ClusterResourceNames,
            types::{
                kubernetes::Uid,
                operator::{
                    ClusterName, ControllerName, OperatorName, ProductName, ProductVersion,
                },
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

    impl NameIsValidLabelValue for Cluster {
        fn to_label_value(&self) -> String {
            self.to_name()
        }
    }

    fn resource_names() -> ClusterResourceNames {
        ClusterResourceNames {
            cluster_name: ClusterName::from_str_unsafe("cluster-name"),
            product_name: ProductName::from_str_unsafe("my-product"),
        }
    }

    fn context_names() -> ContextNames {
        ContextNames {
            product_name: ProductName::from_str_unsafe("my-product"),
            operator_name: OperatorName::from_str_unsafe("product.example.org"),
            controller_name: ControllerName::from_str_unsafe("productcluster"),
        }
    }

    fn product_version() -> ProductVersion {
        ProductVersion::from_str_unsafe("1.2.3")
    }

    fn expected_metadata(name: &str) -> ObjectMeta {
        ObjectMeta {
            labels: Some(
                [
                    ("app.kubernetes.io/component", "none"),
                    ("app.kubernetes.io/instance", "cluster-name"),
                    (
                        "app.kubernetes.io/managed-by",
                        "product.example.org_productcluster",
                    ),
                    ("app.kubernetes.io/name", "my-product"),
                    ("app.kubernetes.io/role-group", "none"),
                    ("app.kubernetes.io/version", "1.2.3"),
                    ("stackable.tech/vendor", "Stackable"),
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
            build_service_account(
                &Cluster::new(),
                &resource_names(),
                &context_names(),
                &product_version(),
            )
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
            build_role_binding(
                &Cluster::new(),
                &resource_names(),
                &context_names(),
                &product_version(),
            )
        );
    }

    /// The v2 output must stay byte-identical to the v1 builder given the same labels, so that
    /// migrating an operator from v1 to v2 changes nothing but where the labels come from.
    #[test]
    #[allow(deprecated)]
    fn output_is_identical_to_v1_build_rbac_resources() {
        let cluster = Cluster::new();

        let names = context_names();
        let version = product_version();
        let labels = recommended_labels(
            &cluster,
            &names.product_name,
            &version,
            &names.operator_name,
            &names.controller_name,
            &NONE_ROLE_NAME,
            &NONE_ROLE_GROUP_NAME,
        );

        let (v1_service_account, v1_role_binding) =
            crate::commons::rbac::build_rbac_resources(&cluster, "my-product", labels)
                .expect("should build the v1 RBAC resources");

        assert_eq!(
            v1_service_account,
            build_service_account(&cluster, &resource_names(), &names, &version)
        );
        assert_eq!(
            v1_role_binding,
            build_role_binding(&cluster, &resource_names(), &names, &version)
        );
    }
}
