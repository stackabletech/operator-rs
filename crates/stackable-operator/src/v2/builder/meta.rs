use stackable_operator::{
    builder::meta::OwnerReferenceBuilder,
    k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference, kube::Resource,
};

use crate::framework::{HasName, HasUid};

/// Infallible variant of
/// [`stackable_operator::builder::meta::ObjectMetaBuilder::ownerreference_from_resource`]
pub fn ownerreference_from_resource(
    resource: &(impl Resource<DynamicType = ()> + HasName + HasUid),
    block_owner_deletion: Option<bool>,
    controller: Option<bool>,
) -> OwnerReference {
    OwnerReferenceBuilder::new()
        // Set api_version, kind, name and additionally the UID if it exists.
        .initialize_from_resource(resource)
        // Ensure that the name is set.
        .name(resource.to_name())
        // Ensure that the UID is set.
        .uid(resource.to_uid().to_string())
        .block_owner_deletion_opt(block_owner_deletion)
        .controller_opt(controller)
        .build()
        .expect(
            "OwnerReference should be created because the resource has an api_version, kind, name \
            and uid.",
        )
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use stackable_operator::{
        k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference},
        kube::Resource,
    };

    use crate::framework::{HasName, HasUid, Uid, builder::meta::ownerreference_from_resource};

    struct Cluster {
        object_meta: ObjectMeta,
    }

    impl Cluster {
        fn new() -> Self {
            Cluster {
                object_meta: ObjectMeta {
                    name: Some("cluster-name".to_owned()),
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

    #[test]
    fn test_ownerreference_from_resource() {
        let actual_owner_reference =
            ownerreference_from_resource(&Cluster::new(), Some(true), Some(true));

        let expected_owner_reference = OwnerReference {
            api_version: "group/version".to_owned(),
            block_owner_deletion: Some(true),
            controller: Some(true),
            kind: "kind".to_owned(),
            name: "cluster-name".to_owned(),
            uid: "a6b89911-d48e-4328-88d6-b9251226583d".to_owned(),
        };

        assert_eq!(expected_owner_reference, actual_owner_reference);
    }
}
