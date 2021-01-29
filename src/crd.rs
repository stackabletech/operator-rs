use crate::client::Client;
use crate::error::{Error, OperatorResult};

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::error::ErrorResponse;
use tracing::info;

/// This trait can be implemented to allow automatic handling
/// (e.g. creation) of `CustomResourceDefinition`s in Kubernetes.
pub trait CRD {
    /// The name of the Resource in Kubernetes
    ///
    /// # Example
    ///
    /// ```no_run
    /// const RESOURCE_NAME: &'static str = "zookeeperclusters.zookeeper.stackable.de";
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
/// # use stackable_operator::{CRD, create_client};
/// #
/// # struct Test;
/// # impl CRD for Test {
/// #    const RESOURCE_NAME: &'static str = "foo.bar.com";
/// #    const CRD_DEFINITION: &'static str = "mycrdhere";
/// # }
/// #
/// # async {
/// # let client = create_client(Some("foo".to_string())).await.unwrap();
/// use stackable_operator::crd::exists;
/// exists::<Test>(client).await;
/// # };
/// ```
pub async fn exists<T>(client: Client) -> OperatorResult<bool>
where
    T: CRD,
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

/// This makes sure the CRD is registered in the apiserver.
/// Currently this does not retry internally.
/// This means that running it again _might_ work in case of transient errors.
// TODO: Make sure to wait until it's enabled in the apiserver
pub async fn ensure_crd_created<T>(client: Client) -> OperatorResult<()>
where
    T: CRD,
{
    if exists::<T>(client.clone()).await? {
        info!("CRD already exists in the cluster");
        Ok(())
    } else {
        info!("CRD not detected in Kubernetes. Attempting to create it.");
        create::<T>(client).await
        // TODO: Maybe retry?
    }
}

/// Creates the CRD in the Kubernetes cluster.
/// It will return an error if the CRD already exists.
/// If it returns successfully it does not mean that the CRD is fully established yet,
/// just that it has been accepted by the apiserver.
async fn create<T>(client: Client) -> OperatorResult<()>
where
    T: CRD,
{
    let zk_crd: CustomResourceDefinition = serde_yaml::from_str(T::CRD_DEFINITION)?;
    client.create(&zk_crd).await.and(Ok(()))
}
