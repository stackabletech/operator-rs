use std::time::Duration;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use tracing::{debug, error, info, warn};

use crate::client::Client;
use crate::error::Error::RequiredCrdsMissing;
use crate::error::OperatorResult;
use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use kube::api::ListParams;
use std::collections::HashSet;

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
pub async fn ensure_crd_created<T>(client: &Client) -> OperatorResult<()>
where
    T: Crd,
{
    if client
        .exists::<CustomResourceDefinition>(T::RESOURCE_NAME, None)
        .await?
    {
        info!("CRD already exists in the cluster");
        Ok(())
    } else {
        info!("CRD not detected in Kubernetes. Attempting to create it.");

        loop {
            if let Ok(res) = create::<T>(client).await {
                break res;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        wait_created::<T>(client).await?;
        Ok(())
    }
}

/// Checks if a list of CRDs exists in Kubernetes, does not attempt to create missing CRDs.
///
/// If not all specified CRDs are present the function will keep checking regularly until a
/// specified timeout is reached (or indefinitely).
///
/// This is intended to be used in pre-flight checks of operators to ensure that all CRDs they
/// require to work properly are present in Kubernetes.
///
/// # Parameters
/// - `client`: Client to connect to Kubernetes API and create the CRD with.
/// - `names`: The list of CRDs to check
/// - `delay`: If specified, waits for the given `Duration` before checking again if all
///     CRDs are present. If not specified defaults to 60 seconds.
/// - `timeout`: If specified, keeps checking for the given `Duration`. If not specified,
///     retries indefinitely.
pub async fn wait_until_crds_present(
    client: &Client,
    names: Vec<&str>,
    timeout: Option<Duration>,
) -> OperatorResult<()> {
    let mut backoff_strategy = ExponentialBackoff {
        max_elapsed_time: timeout,
        ..ExponentialBackoff::default()
    };

    // The loop will continue running until either all CRDs are present or a configured
    // timeout is reached
    loop {
        debug!(
            "Checking if the following CRDs have been created: {:?}",
            names
        );

        // Concurrently use `check_crd` to check if CRDs are there, this returns a Result containing
        // a tuple (crd_name, presence_flag) which is collected into a single result we can then
        // check
        // If any requests to Kubernetes fail (crd missing is not considered a failure here) the
        // remaining futures are aborted, as we wouldn't be able to use the results anyway
        let check_result = futures::future::try_join_all(
            names
                .iter()
                .map(|crd_name| check_crd(client, crd_name))
                .collect::<Vec<_>>(),
        )
        .await
        // Any error returned here was an error when talking to Kubernetes and will mark this
        // entire iteration as failed
        .and_then(|crd_results| {
            debug!("Received results for CRD presence check: {:?}", crd_results);
            let missing_crds = crd_results
                .iter()
                .filter(|(_, present)| !*present)
                .map(|(name, _)| String::from(name))
                .collect::<HashSet<_>>();
            if missing_crds.is_empty() {
                Ok(())
            } else {
                Err(RequiredCrdsMissing {
                    names: missing_crds,
                })
            }
        });

        // Checks done, now we
        //   1. return ok(()) if all CRDs are present
        //   2. return an error if CRDs are missing and the timeout has expired
        //   3. queue another loop iteration if an error occurred and the timeout has not expired
        match check_result {
            Ok(()) => return Ok(()),
            Err(err) => {
                match &err {
                    RequiredCrdsMissing { names } => warn!(
                        "The following required CRDs are missing in Kubernetes: [{:?}]",
                        names
                    ),
                    err => error!(
                        "Error occurred when checking if all required CRDs are present: [{}]",
                        err
                    ),
                }

                // When backoff returns `None` the timeout has expired
                match backoff_strategy.next_backoff() {
                    Some(backoff) => {
                        info!(
                            "Waiting [{}] seconds before trying again..",
                            backoff.as_secs()
                        );
                        tokio::time::sleep(backoff).await;
                    }
                    None => {
                        info!(
                            "Waiting for CRDs timed out after [{}] seconds.",
                            backoff_strategy
                                .max_elapsed_time
                                .unwrap_or_else(|| Duration::from_secs(0))
                                .as_secs()
                        );
                        return Err(err);
                    }
                }
            }
        };
    }
}

async fn check_crd(client: &Client, crd_name: &str) -> OperatorResult<(String, bool)> {
    Ok((
        crd_name.to_string(),
        client
            .exists::<CustomResourceDefinition>(crd_name, None)
            .await?,
    ))
}

/// Creates the CRD in the Kubernetes cluster.
/// It will return an error if the CRD already exists.
/// If it returns successfully it does not mean that the CRD is fully established yet,
/// just that it has been accepted by the apiserver.
async fn create<T>(client: &Client) -> OperatorResult<()>
where
    T: Crd,
{
    let crd: CustomResourceDefinition = serde_yaml::from_str(T::CRD_DEFINITION)?;
    client.create(&crd).await.and(Ok(()))
}

/// Waits until CRD of given type `T` is applied to Kubernetes.
pub async fn wait_created<T>(client: &Client) -> OperatorResult<()>
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
