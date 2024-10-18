use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscaler;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, CustomResource, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[kube(
    group = "hdfs.stackable.tech",
    version = "v1alpha1",
    kind = "RoleGroupScaler",
    plural = "rolegroupsscalers",
    shortname = "scaler",
    status = "ScalerStatus",
    scale = r#"{"specReplicasPath":".spec.replicas", "statusReplicasPath":".status.replicas", "labelSelectorPath":".spec.labelSelector"}"#,
    namespaced,
    crates(
        kube_core = "kube::core",
        k8s_openapi = "k8s_openapi",
        schemars = "schemars"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct RoleGroupScalerSpec {
    pub replicas: Option<u16>,
    pub label_selector: String,
}

#[derive(Clone, Default, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalerStatus {
    pub replicas: u8,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ScalingConfig {
    Static { replicas: u16},
    AutoScaling {hpa: HorizontalPodAutoscaler},
}