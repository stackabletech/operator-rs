use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::error::ErrorResponse;
use tracing::info;

use crate::client::Client;
use crate::error::{Error, OperatorResult};
use kube::api::ListParams;

/// This trait can be implemented to allow automatic handling
/// (e.g. creation) of `CustomResourceDefinition`s in Kubernetes.
pub trait Crd {
    /// The name of the Resource in Kubernetes
    ///
    /// # Example
    ///
    /// ```no_run
    /// const RESOURCE_NAME: &'static str = "foo.bar.stackable.tech";
    /// ```
    const RESOURCE_NAME: &'static str;

    /// The full YAML definition of the CRD.
    /// In theory this can be generated from the structs itself but the kube-rs library
    /// we use currently does not generate the required [schema](https://github.com/clux/kube-rs/issues/264)
    /// and it also has no support for [validation](https://github.com/clux/kube-rs/issues/129)
    const CRD_DEFINITION: &'static str;
}

/// Returns Ok(true) if our CRD has been registered in Kubernetes, Ok(false) if it could not be found
/// and Error in any other case (e.g. connection to Kubernetes failed in some way.
///
/// # Example
///
/// ```no_run
/// # use stackable_operator::Crd;
/// # use stackable_operator::client;
/// #
/// # struct Test;
/// # impl Crd for Test {
/// #    const RESOURCE_NAME: &'static str = "foo.bar.stackable.tech";
/// #    const CRD_DEFINITION: &'static str = "mycrdhere";
/// # }
/// #
/// # async {
/// # let client = client::create_client(Some("foo".to_string())).await.unwrap();
/// use stackable_operator::crd::exists;
/// exists::<Test>(client).await;
/// # };
/// ```
pub async fn exists<T>(client: Client) -> OperatorResult<bool>
where
    T: Crd,
{
    match client
        .get::<CustomResourceDefinition>(T::RESOURCE_NAME, None)
        .await
    {
        Ok(_) => Ok(true),
        Err(Error::KubeError {
            source: kube::error::Error::Api(ErrorResponse { reason, .. }),
        }) if reason == "NotFound" => Ok(false),
        Err(err) => Err(err),
    }
}

/// Makes sure CRD of given type `T` is running and accepted by the Kubernetes apiserver.
/// If the CRD already exists at the time this method is invoked, this method exits.
/// If there is no CRD of type `T` yet, it will attempt to create it and verify k8s apiserver
/// applied the CRD.
///
/// # Parameters
/// - `client`: Client to connect to Kubernetes API and create the CRD with
/// - `timeout`: If specified, retries creating the CRD for given `Duration`. If not specified,
///     retries indefinitely.
pub async fn ensure_crd_created<T>(client: Client, timeout: Option<Duration>) -> OperatorResult<()>
where
    T: Crd,
{
    if exists::<T>(client.clone()).await? {
        info!("CRD already exists in the cluster");
        Ok(())
    } else {
        info!("CRD not detected in Kubernetes. Attempting to create it.");

        let crd_created = async {
            loop {
                if let Ok(res) = create::<T>(client.clone()).await {
                    break res;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            wait_ready::<T>(client.clone()).await?;
            Ok(())
        };
        if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, crd_created).await?
        } else {
            crd_created.await
        }
    }
}

/// Creates the CRD in the Kubernetes cluster.
/// It will return an error if the CRD already exists.
/// If it returns successfully it does not mean that the CRD is fully established yet,
/// just that it has been accepted by the apiserver.
async fn create<T>(client: Client) -> OperatorResult<()>
where
    T: Crd,
{
    let crd: CustomResourceDefinition = serde_yaml::from_str(T::CRD_DEFINITION)?;
    client.create(&crd).await.and(Ok(()))
}

/// Waits until CRD of given type `T` is applied to Kubernetes.
pub async fn wait_ready<T>(client: Client) -> OperatorResult<()>
where
    T: Crd,
{
    let lp: ListParams = ListParams {
        field_selector: Some(format!("metadata.name={}", T::RESOURCE_NAME)),
        ..ListParams::default()
    };
    client
        .wait_ready::<CustomResourceDefinition>(None, lp)
        .await;
    Ok(())
}
