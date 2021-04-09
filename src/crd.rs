use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::error::ErrorResponse;
use tracing::info;

use crate::client::Client;
use crate::error::{Error, OperatorResult};
use kube::api::ListParams;
use std::fs::File;
use std::io::Write;
use std::path::Path;

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

    /// Returns a [`CustomResourceDefinition`] for this resource.
    ///
    /// # Implementation note
    ///
    /// When using the [`CustomResource`] derive you'll get a `crd()` method automatically.
    /// All you need to do is to forward to this method.
    ///
    /// ## Example
    ///
    /// ```text
    ///     fn crd() -> CustomResourceDefinition {
    ///         MyCustomResource::crd()
    ///     }     
    ///
    /// ```
    fn crd() -> CustomResourceDefinition;

    /// Generates a YAML CustomResourceDefinition and writes it to a `Write`r.
    fn generate_yaml_schema<W>(mut writer: W) -> OperatorResult<()>
    where
        W: Write,
    {
        let schema = serde_yaml::to_string(&Self::crd())?;
        writer.write_all(schema.as_bytes())?;
        Ok(())
    }

    /// Generates a YAML CustomResourceDefinition and writes it to the specified file.
    fn write_yaml_schema<P: AsRef<Path>>(path: P) -> OperatorResult<()> {
        let writer = File::create(path)?;
        Self::generate_yaml_schema(writer)
    }

    /// Generates a YAML CustomResourceDefinition and prints it to stdout.
    fn print_yaml_schema() -> OperatorResult<()> {
        let writer = std::io::stdout();
        Self::generate_yaml_schema(writer)
    }
}

/// Returns Ok(true) if our CRD has been registered in Kubernetes, Ok(false) if it could not be found
/// and Error in any other case (e.g. connection to Kubernetes failed in some way.
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
/// applied the CRD. This method retries indefinitely. Use timeout on the `future` returned
/// to apply time limit constraint.
///
/// # Parameters
/// - `client`: Client to connect to Kubernetes API and create the CRD with
/// - `timeout`: If specified, retries creating the CRD for given `Duration`. If not specified,
///     retries indefinitely.
pub async fn ensure_crd_created<T>(client: Client) -> OperatorResult<()>
where
    T: Crd,
{
    if exists::<T>(client.clone()).await? {
        info!("CRD already exists in the cluster");
        Ok(())
    } else {
        info!("CRD not detected in Kubernetes. Attempting to create it.");

        loop {
            if let Ok(res) = create::<T>(client.clone()).await {
                break res;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        wait_created::<T>(client.clone()).await?;
        Ok(())
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
    client.create(&T::crd()).await.and(Ok(()))
}

/// Waits until CRD of given type `T` is applied to Kubernetes.
pub async fn wait_created<T>(client: Client) -> OperatorResult<()>
where
    T: Crd,
{
    let lp: ListParams = ListParams {
        field_selector: Some(format!("metadata.name={}", T::RESOURCE_NAME)),
        ..ListParams::default()
    };
    client
        .wait_created::<CustomResourceDefinition>(None, lp)
        .await;
    Ok(())
}
