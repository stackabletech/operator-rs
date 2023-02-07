use crate::client::Client;
use crate::cluster_resources::ClusterResources;
use crate::error::Error::ReconciliationAborted;
use crate::error::OperatorResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Serialize, Deserialize, Eq, PartialEq, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClusterSpecCommons {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation_paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_type: Option<ServiceType>,
}

impl ClusterSpecCommons {
    pub fn stopped(&self) -> bool {
        self.stopped.unwrap_or(false)
    }

    pub fn reconciliation_paused(&self) -> bool {
        self.reconciliation_paused.unwrap_or(false)
    }

    pub fn service_type(&self) -> ServiceType {
        self.service_type.clone().unwrap_or(ServiceType::ClusterIP)
    }
}

#[derive(Clone, Debug, Display, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum ServiceType {
    NodePort,
    ClusterIP,
}

pub async fn handle_common_flags(
    client: &Client,
    cluster_resources: &ClusterResources,
    flags: &ClusterSpecCommons,
) -> OperatorResult<()> {
    if flags.reconciliation_paused() {
        tracing::info!("Reconciliation for this cluster has been paused, aborting ..");
        return Err(ReconciliationAborted {
            message: "Reconciliation for this cluster has been paused".to_string(),
        });
    };

    // Check if the CRD has the annotation to signify that the cluster is stopped
    if flags.stopped() {
        tracing::info!("Cluster has stopped annotation..");

        if cluster_resources
            .stop_deployed_cluster_resources(&client)
            .await?
        {
            tracing::info!("Stopped all cluster resources.")
        } else {
            tracing::info!("Cluster already fully stopped, not doing anything.")
        }
        return Err(ReconciliationAborted {
            message: "Cluster has stopped flag set".to_string(),
        });
    }
    Ok(())
}
