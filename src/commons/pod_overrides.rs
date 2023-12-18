use k8s_openapi::api::core::v1::PodTemplateSpec;
use schemars::{schema::Schema, visit::Visitor, JsonSchema};

/// Simplified schema for PodTemplateSpec without mandatory fields (e.g. `containers`) or documentation.
///
/// The normal PodTemplateSpec requires you to specify `containers` as an `Vec<Container>`.
/// Often times the user want's to overwrite/add stuff not related to a container
/// (e.g. tolerations or a ServiceAccount), so it's annoying that he always needs to
/// specify an empty array for `containers`.
///
/// Additionally all docs are removed, as the resulting Stackable CRD objects where to big for Kubernetes.
/// E.g. the HdfsCluster CRD increased to ~3.2 MB (which is over the limit of 3MB), after stripping
/// the docs it went down to ~1.3 MiB.
pub fn pod_overrides_schema(gen: &mut schemars::gen::SchemaGenerator) -> Schema {
    let mut schema = PodTemplateSpec::json_schema(gen);
    SimplifyOverrideSchema.visit_schema(&mut schema);

    if let Schema::Object(schema) = &mut schema {
        let meta = schema.metadata.get_or_insert_with(Default::default);
        meta.description = Some("See PodTemplateSpec (https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.27/#podtemplatespec-v1-core) for more details".to_string());
    }

    schema
}

struct SimplifyOverrideSchema;
impl schemars::visit::Visitor for SimplifyOverrideSchema {
    fn visit_schema_object(&mut self, schema: &mut schemars::schema::SchemaObject) {
        // Strip docs to make the schema more compact
        if let Some(meta) = &mut schema.metadata {
            meta.description = None;
            meta.examples.clear();
        }

        // Make all options optional
        if let Some(object) = &mut schema.object {
            object.required.clear();
        }

        schemars::visit::visit_schema_object(self, schema);
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Test {
        #[schemars(schema_with = "pod_overrides_schema")]
        pub pod_overrides: PodTemplateSpec,
    }

    #[test]
    fn test_valid_pod_override_with_tolerations() {
        let input = r#"
          podOverrides:
            spec:
              tolerations:
                - key: "key1"
                  operator: "Equal"
                  value: "value1"
                  effect: "NoSchedule"
        "#;

        serde_yaml::from_str::<Test>(input).expect("Failed to parse valid podOverride");
    }

    #[test]
    fn test_valid_pod_override_with_labels() {
        let input = r#"
          podOverrides:
            metadata:
              labels:
                my-custom-label: super-important-label
        "#;

        serde_yaml::from_str::<Test>(input).expect("Failed to parse valid podOverride");
    }

    #[test]
    fn test_valid_pod_override_with_containers() {
        let input = r#"
          podOverrides:
            spec:
              containers:
                - name: nifi
                  command:
                    - "tail"
                    - "-f"
                    - "/dev/null"
        "#;

        serde_yaml::from_str::<Test>(input).expect("Failed to parse valid podOverride");
    }

    #[test]
    fn test_valid_pod_override_with_containers_and_volumes() {
        let input = r#"
          podOverrides:
            spec:
              containers:
                - name: nifi
                  image: docker.stackable.tech/stackable/nifi:1.23.2-stackable23.11.0
                  volumeMounts:
                    - name: jar
                      mountPath: /stackable/nifi/lib/wifi.png
                  command:
                    - "tail"
                    - "-f"
                    - "/dev/null"
              volumes:
                - name: jar
                  persistentVolumeClaim:
                    claimName: nifi-jar
        "#;

        serde_yaml::from_str::<Test>(input).expect("Failed to parse valid podOverride");
    }

    #[test]
    fn test_invalid_pod_override_missing_container_name() {
        let input = r#"
          podOverrides:
            spec:
              containers:
                - image: docker.stackable.tech/stackable/nifi:1.23.2-stackable23.11.0
        "#;

        // FIXME: Ideally we would require the names of the containers to be set.  We had users using podOverrides
        // without setting the name of the container and wondering why it didn't work.
        serde_yaml::from_str::<Test>(input).expect("Failed to parse valid podOverride");
    }
}
