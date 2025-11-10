use k8s_openapi::DeepMerge;
use kube::core::DynamicObject;
use serde::{Deserialize, de::DeserializeOwned};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to deserialize dynamic object"))]
    DeserializeDynamicObject { source: serde_yaml::Error },

    #[snafu(display(
        "failed to parse dynamic object as apiVersion {target_api_version:?} and kind {target_kind:?}"
    ))]
    ParseDynamicObject {
        source: kube::core::dynamic::ParseDynamicObjectError,
        target_api_version: String,
        target_kind: String,
    },
}

pub fn parse_patches(patch: impl AsRef<str>) -> Result<Vec<DynamicObject>, Error> {
    serde_yaml::Deserializer::from_str(patch.as_ref())
        .map(|manifest| DynamicObject::deserialize(manifest).context(DeserializeDynamicObjectSnafu))
        .collect()
}

pub fn apply_patches<'a, R>(
    base: &mut R,
    patches: impl Iterator<Item = &'a DynamicObject>,
) -> Result<(), Error>
where
    R: kube::Resource<DynamicType = ()> + DeepMerge + DeserializeOwned,
{
    for patch in patches {
        apply_patch(base, patch)?;
    }
    Ok(())
}

pub fn apply_patch<R>(base: &mut R, patch: &DynamicObject) -> Result<(), Error>
where
    R: kube::Resource<DynamicType = ()> + DeepMerge + DeserializeOwned,
{
    use kube::ResourceExt;

    let Some(patch_type) = &patch.types else {
        return Ok(());
    };
    if patch_type.api_version != R::api_version(&()) || patch_type.kind != R::kind(&()) {
        return Ok(());
    }
    let Some(patch_name) = &patch.metadata.name else {
        return Ok(());
    };

    // The name always needs to match
    if &base.name_any() != patch_name {
        return Ok(());
    }

    // If there is a namespace on the base object, it needs to match as well
    // Note that it is not set for cluster-scoped objects.
    if base.namespace() != patch.metadata.namespace {
        return Ok(());
    }

    let deserialized_patch =
        patch
            .clone()
            .try_parse()
            .with_context(|_| ParseDynamicObjectSnafu {
                target_api_version: R::api_version(&()),
                target_kind: R::kind(&()),
            })?;
    base.merge_from(deserialized_patch);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, vec};

    use k8s_openapi::{
        ByteString, Metadata,
        api::{
            apps::v1::{
                RollingUpdateStatefulSetStrategy, StatefulSet, StatefulSetSpec,
                StatefulSetUpdateStrategy,
            },
            core::v1::{
                ConfigMap, Container, ContainerPort, PodSpec, PodTemplateSpec, Secret,
                ServiceAccount,
            },
            storage::v1::StorageClass,
        },
        apimachinery::pkg::util::intstr::IntOrString,
    };
    use kube::api::ObjectMeta;

    use super::*;

    fn generate_service_account() -> ServiceAccount {
        serde_yaml::from_str(
            "
apiVersion: v1
kind: ServiceAccount
metadata:
  name: trino-serviceaccount
  namespace: default
  labels:
    app.kubernetes.io/instance: trino
    app.kubernetes.io/managed-by: trino.stackable.tech_trinocluster
    app.kubernetes.io/name: trino
  ownerReferences:
  - apiVersion: trino.stackable.tech/v1alpha1
    controller: true
    kind: TrinoCluster
    name: trino
    uid: c85bfb53-a28e-4782-baaf-3c218a25f192
",
        )
        .unwrap()
    }

    fn generate_stateful_set() -> StatefulSet {
        StatefulSet {
            metadata: generate_metadata("trino-coordinator-default"),
            spec: Some(StatefulSetSpec {
                service_name: Some("trino-coordinator-default".to_owned()),
                update_strategy: Some(StatefulSetUpdateStrategy {
                    rolling_update: Some(RollingUpdateStatefulSetStrategy {
                        max_unavailable: Some(IntOrString::Int(42)),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(generate_labels()),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "trino".to_owned(),
                            image: Some("trino-image".to_owned()),
                            ports: Some(vec![ContainerPort {
                                container_port: 8443,
                                name: Some("https".to_owned()),
                                protocol: Some("https".to_owned()),
                                ..Default::default()
                            }]),
                            ..Default::default()
                        }],
                        service_account_name: Some("trino-serviceaccount".to_owned()),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn generate_metadata(name: impl Into<String>) -> ObjectMeta {
        ObjectMeta {
            name: Some(name.into()),
            namespace: Some("default".to_owned()),
            labels: Some(generate_labels()),
            ..Default::default()
        }
    }

    fn generate_labels() -> BTreeMap<String, String> {
        BTreeMap::from([("app.kubernetes.io/name".to_owned(), "trino".to_owned())])
    }

    #[test]
    fn service_account_patched() {
        let mut sa = generate_service_account();
        let patches = parse_patches(
            "
apiVersion: v1
kind: ServiceAccount
metadata:
  name: trino-serviceaccount
  namespace: default
  labels:
    app.kubernetes.io/name: overwritten
    foo: bar
",
        )
        .unwrap();

        assert_has_label(&sa, "app.kubernetes.io/name", "trino");
        apply_patches(&mut sa, patches.iter()).unwrap();
        assert_has_label(&sa, "app.kubernetes.io/name", "overwritten");
    }

    #[test]
    fn service_account_not_patched_as_different_name() {
        let mut sa = generate_service_account();
        let patches = parse_patches(
            "
apiVersion: v1
kind: ServiceAccount
metadata:
  name: other-sa
  namespace: default
  labels:
    app.kubernetes.io/name: overwritten
    foo: bar
",
        )
        .unwrap();

        let original = sa.clone();
        apply_patches(&mut sa, patches.iter()).unwrap();
        assert_eq!(sa, original, "The patch shouldn't have changed anything");
    }

    #[test]
    fn service_account_not_patched_as_different_namespace() {
        let mut sa = generate_service_account();
        let patches = parse_patches(
            "
apiVersion: v1
kind: ServiceAccount
metadata:
  name: trino-serviceaccount
  namespace: other-namespace
  labels:
    app.kubernetes.io/name: overwritten
    foo: bar
",
        )
        .unwrap();

        let original = sa.clone();
        apply_patches(&mut sa, patches.iter()).unwrap();
        assert_eq!(sa, original, "The patch shouldn't have changed anything");
    }

    #[test]
    fn service_account_not_patched_as_different_api_version() {
        let mut sa = generate_service_account();
        let patches = parse_patches(
            "
apiVersion: v42
kind: ServiceAccount
metadata:
  name: trino-serviceaccount
  namespace: default
  labels:
    app.kubernetes.io/name: overwritten
    foo: bar
",
        )
        .unwrap();

        let original = sa.clone();
        apply_patches(&mut sa, patches.iter()).unwrap();
        assert_eq!(sa, original, "The patch shouldn't have changed anything");
    }

    #[test]
    fn statefulset_patched_multiple_patches() {
        let mut sts = generate_stateful_set();

        let patches = parse_patches(
            "
apiVersion: v1
kind: ServiceAccount
metadata:
  name: trino-serviceaccount
  namespace: default
  labels:
    app.kubernetes.io/name: overwritten
    foo: bar
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: trino-coordinator-default
  namespace: default
spec:
  template:
    metadata:
      labels:
        foo: bar
    spec:
      containers:
      - name: trino
        image: custom-image
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: trino-coordinator-default
  namespace: default
spec:
  replicas: 3
",
        )
        .unwrap();

        let get_replicas = |sts: &StatefulSet| sts.spec.as_ref().unwrap().replicas;
        let get_trino_container = |sts: &StatefulSet| {
            sts.spec
                .as_ref()
                .unwrap()
                .template
                .spec
                .as_ref()
                .unwrap()
                .containers
                .iter()
                .find(|c| c.name == "trino")
                .unwrap()
                .clone()
        };
        let get_trino_container_image = |sts: &StatefulSet| get_trino_container(sts).image;

        assert_eq!(get_replicas(&sts), None);
        assert_eq!(
            get_trino_container_image(&sts).as_deref(),
            Some("trino-image")
        );
        apply_patches(&mut sts, patches.iter()).unwrap();
        assert_eq!(get_replicas(&sts), Some(3));
        assert_eq!(
            get_trino_container_image(&sts).as_deref(),
            Some("custom-image")
        );
    }

    #[test]
    fn configmap_patched() {
        let mut cm: ConfigMap = serde_yaml::from_str(
            "
apiVersion: v1
kind: ConfigMap
metadata:
  name: game-demo
data:
  foo: bar
  config.properties: |-
    coordinator=true
    http-server.https.enabled=true
  log.properties: |-
    =info
",
        )
        .unwrap();
        let patches = parse_patches(
            "
apiVersion: v1
kind: ConfigMap
metadata:
  name: game-demo
data:
  foo: overwritten
  log.properties: |-
    =info,tech.stackable=debug
",
        )
        .unwrap();

        assert_eq!(
            cm.data.as_ref().unwrap(),
            &BTreeMap::from([
                ("foo".to_owned(), "bar".to_owned()),
                (
                    "config.properties".to_owned(),
                    "coordinator=true\nhttp-server.https.enabled=true".to_owned()
                ),
                ("log.properties".to_owned(), "=info".to_owned()),
            ])
        );
        apply_patches(&mut cm, patches.iter()).unwrap();
        assert_eq!(
            cm.data.as_ref().unwrap(),
            &BTreeMap::from([
                ("foo".to_owned(), "overwritten".to_owned()),
                (
                    "config.properties".to_owned(),
                    "coordinator=true\nhttp-server.https.enabled=true".to_owned()
                ),
                (
                    "log.properties".to_owned(),
                    "=info,tech.stackable=debug".to_owned()
                ),
            ])
        );
    }

    #[test]
    fn secret_patched() {
        let mut secret: Secret = serde_yaml::from_str(
            "
apiVersion: v1
kind: Secret
metadata:
  name: dotfile-secret
stringData:
  foo: bar
data:
  raw: YmFyCg== # echo bar | base64
",
        )
        .unwrap();
        let patches = parse_patches(
            "
apiVersion: v1
kind: Secret
metadata:
  name: dotfile-secret
stringData:
  foo: overwritten
data:
  raw: b3ZlcndyaXR0ZW4K # echo overwritten | base64
",
        )
        .unwrap();

        assert_eq!(
            secret.string_data.as_ref().unwrap(),
            &BTreeMap::from([("foo".to_owned(), "bar".to_owned())])
        );
        assert_eq!(
            secret.data.as_ref().unwrap(),
            &BTreeMap::from([("raw".to_owned(), ByteString(b"bar\n".to_vec()))])
        );

        apply_patches(&mut secret, patches.iter()).unwrap();
        assert_eq!(
            secret.string_data.as_ref().unwrap(),
            &BTreeMap::from([("foo".to_owned(), "overwritten".to_owned()),])
        );
        assert_eq!(
            secret.data.as_ref().unwrap(),
            &BTreeMap::from([("raw".to_owned(), ByteString(b"overwritten\n".to_vec()))])
        );
    }

    #[test]
    fn cluster_scoped_object_patched() {
        let mut storage_class: StorageClass = serde_yaml::from_str(
            "
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: low-latency
  labels:
    foo: original
  annotations:
    storageclass.kubernetes.io/is-default-class: \"false\"
provisioner: csi-driver.example-vendor.example
",
        )
        .unwrap();
        let patches = parse_patches(
            "
---
apiVersion: v1
kind: ServiceAccount
---
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: low-latency
  labels:
    foo: overwritten
  annotations:
    new: annotation
provisioner: custom-provisioner
---
foo: bar
",
        )
        .unwrap();

        assert_has_label(&storage_class, "foo", "original");
        apply_patches(&mut storage_class, patches.iter()).unwrap();
        assert_has_label(&storage_class, "foo", "overwritten");
    }

    fn assert_has_label<O: Metadata<Ty = ObjectMeta>>(
        object: &O,
        key: impl AsRef<str>,
        value: impl AsRef<str>,
    ) {
        assert_eq!(
            object
                .metadata()
                .labels
                .as_ref()
                .expect("labels missing")
                .get(key.as_ref())
                .expect("key missing from labels"),
            value.as_ref()
        );
    }
}
