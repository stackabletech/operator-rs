use crate::error;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::PostParams;
use kube::{Client, Api};
use tracing::info;
use kube::error::ErrorResponse;

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
/// use stackable_operator::crd::exists;
///
/// exists::<Test>(client);
/// ```
pub async fn exists<T>(client: Client) -> Result<bool, error::Error>
    where T: CRD
{
    let api: Api<CustomResourceDefinition> = Api::all(client);

    match api.get(T::RESOURCE_NAME).await {
        Ok(_) => { Ok(true) }
        Err(kube::error::Error::Api(ErrorResponse {reason, .. })) if reason == "NotFound" => { println!("foo"); Ok(false) }
        Err(err) => Err(error::Error::from(err))
    }
}

/// This makes sure the CRD is registered in the apiserver.
/// This will panic if there is an error.
// TODO: Make sure to wait until it's enabled in the apiserver
pub async fn ensure_crd_created<T>(client: Client)
    where T: CRD
{
    if exists::<T>(client.clone()).await.unwrap() {
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

#[cfg(test)]
mod tests {
    use crate::crd::exists;
    use crate::CRD;

    use kube::CustomResource;
    use serde::{Deserialize, Serialize};
    use tokio_test::assert_err;


    #[derive(Clone, CustomResource, Debug, Deserialize, Serialize)]
    #[kube(
        group = "test.stackable.de",
        version = "v1",
        kind = "Test",
        shortname = "tst",
        namespaced
    )]
    pub struct TestSpec {
        pub name: String
    }

    impl CRD for Test {
        const RESOURCE_NAME: &'static str = "tests.test.stackable.de";
        const CRD_DEFINITION: &'static str = "FOOBAR";
    }

    // TODO:
    #[test]
    fn test_exists()  {
        let client = tokio_test::block_on(kube::Client::try_default()).expect("Client creation should not fail");
        let result = tokio_test::block_on(exists::<Test>(client));
        assert_err!(result);
    }

}
