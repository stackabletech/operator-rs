use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::runtime::wait;
use snafu::{ResultExt, Snafu};
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::watch,
};

use crate::client::Client;

#[derive(Debug, Snafu)]
#[snafu(display("failed to construct signal watcher"))]
pub struct SignalError {
    source: std::io::Error,
}

/// Watches for the incoming signal and multiplies it by sending it to all acquired handles.
pub struct SignalWatcher<T>
where
    T: Send + Sync + 'static,
{
    watch_rx: watch::Receiver<T>,
}

impl<T> SignalWatcher<T>
where
    T: Default + Send + Sync + 'static,
{
    /// Watches the provided `signal` and multiplies the signal by sending it to all acquired handles
    /// constructed through [`SignalWatcher::handle`].
    pub fn new<F>(signal: F) -> Self
    where
        F: Future<Output = T> + Send + Sync + 'static,
    {
        let (watch_tx, watch_rx) = watch::channel(T::default());

        tokio::spawn(async move {
            let value = signal.await;
            watch_tx.send(value)
        });

        Self { watch_rx }
    }

    /// Acquire a new handle which will complete once a `SIGTERM` signal is received.
    ///
    /// This handle can be cheaply cloned to be able to gracefully shutdown multiple concurrent
    /// tasks.
    pub fn handle(&self) -> impl Future<Output = ()> + use<T> {
        let mut watch_rx = self.watch_rx.clone();

        async move {
            watch_rx.changed().await.ok();
        }
    }
}

impl SignalWatcher<()> {
    /// Watches the `SIGTERM` signal and multiplies the signal by sending it to all acquired handlers
    /// constructed through [`SignalWatcher::handle`].
    //
    // NOTE (@Techassi): Note Accepting a generic Future<Output = ()> here is possible, but
    // `Signal::recv` borrows instead of consuming which clashes with the 'static lifetime
    // requirement of `tokio::spawn`. That's why I opted for watching for a particular signal
    // internally instead of requiring users to pass the signal to this function.
    pub fn sigterm() -> Result<Self, SignalError> {
        let mut sigterm = signal(SignalKind::terminate()).context(SignalSnafu)?;
        let (watch_tx, watch_rx) = watch::channel(());

        tokio::spawn(async move {
            sigterm.recv().await;
            watch_tx.send(())
        });

        Ok(Self { watch_rx })
    }
}

#[derive(Debug, Snafu)]
pub enum CrdEstablishedError {
    #[snafu(display(
        "failed to await CRD established condition for {crd_name:?} due to api error"
    ))]
    Api {
        source: kube::runtime::wait::Error,
        crd_name: String,
    },
}

/// Waits for a CRD named `crd_name` to be established.
///
/// The same caveats from [`conditions::is_crd_established`](wait::conditions::is_crd_established)
/// apply here as well.
///
/// ### Errors
///
/// This function returns errors when the underlying API returned errors
/// (CRD is unknown to the Kubernetes API server or due to missing permissions).
pub async fn crd_established(client: &Client, crd_name: &str) -> Result<(), CrdEstablishedError> {
    tracing::info!(
        k8s.crd.name = crd_name,
        "Waiting for the custom resource definition to be established"
    );

    let api: kube::Api<CustomResourceDefinition> = client.get_api(&());
    wait::await_condition(api, crd_name, wait::conditions::is_crd_established())
        .await
        .context(ApiSnafu { crd_name })?;

    tracing::info!(
        k8s.crd.name = crd_name,
        "The custom resource definition is established"
    );

    Ok(())
}
