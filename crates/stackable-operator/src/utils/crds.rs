use schemars::schema::Schema;

pub fn raw_object_schema(_: &mut schemars::gen::SchemaGenerator) -> Schema {
    serde_json::from_value(serde_json::json!({
        "type": "object",
        "x-kubernetes-preserve-unknown-fields": true,
    }))
    .expect("Failed to parse JSON of custom raw object schema")
}

pub fn raw_object_list_schema(_: &mut schemars::gen::SchemaGenerator) -> Schema {
    serde_json::from_value(serde_json::json!({
        "type": "array",
        "items": {
            "type": "object",
            "x-kubernetes-preserve-unknown-fields": true,
        }
    }))
    .expect("Failed to parse JSON of custom raw object list schema")
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::PodTemplateSpec;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Test {
        #[schemars(schema_with = "raw_object_schema")]
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

        // FIXME: Ideally we would require the names of the containers to be set. We had users using podOverrides
        // without setting the name of the container and wondering why it didn't work.
        serde_yaml::from_str::<Test>(input).expect("Failed to parse valid podOverride");
    }
}
