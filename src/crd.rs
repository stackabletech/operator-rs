use crate::error;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::PostParams;
use kube::{Client, Api};
use tracing::info;

pub trait CRD {
    const RESOURCE_NAME: &'static str;
    const CRD_DEFINITION: &'static str;
}

/// Returns true if our CRD has been registered in Kubernetes, false otherwise.
pub async fn exists<T>(client: Client) -> bool
    where T: CRD
{
    let api: Api<CustomResourceDefinition> = Api::all(client);
    return api.get(T::RESOURCE_NAME).await.is_ok(); // TODO: This might also return a transient error (e.g. a timeout)
}

/// This makes sure the CRD is registered in the apiserver.
/// This will panic if there is an error.
// TODO: Make sure to wait until it's enabled in the apiserver
pub async fn ensure_crd_created<T>(client: Client)
    where T: CRD
{
    if exists::<T>(client.clone()).await {
        info!("CRD already exists in the cluster");
    } else {
        info!("CRD not detected in the Kubernetes. Attempting to create it.");
        create::<T>(client)
            .await
            .expect("Creation of CRD should not fail");
        // TODO: Maybe retry?
    }
}

/// Creates the CRD in the Kubernetes cluster.
/// It will return an error if the CRD already exists.
/// If it returns successfully it does not mean that the CRD is fully established yet,
/// just that it has been accepted by the apiserver.
pub async fn create<T>(client: Client) -> Result<(), error::Error>
    where T: CRD
{
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let zk_crd: CustomResourceDefinition = serde_yaml::from_str(T::CRD_DEFINITION)?;
    match api.create(&PostParams::default(), &zk_crd).await {
        Ok(_) => Result::Ok(()),
        Err(err) => Result::Err(error::Error::from(err)),
    }
}
