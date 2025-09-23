use k8s_openapi::{
    ByteString,
    apiextensions_apiserver::pkg::apis::apiextensions::v1::{
        CustomResourceConversion, CustomResourceDefinition, ServiceReference, WebhookClientConfig,
        WebhookConversion,
    },
};
use kube::{
    Api, Client, ResourceExt,
    api::{Patch, PatchParams},
};
use snafu::{ResultExt, Snafu};
use tokio::sync::{mpsc, oneshot};
use x509_cert::{
    Certificate,
    der::{EncodePem, pem::LineEnding},
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to encode CA certificate as PEM format"))]
    EncodeCertificateAuthorityAsPem { source: x509_cert::der::Error },

    #[snafu(display("failed to send initial CRD reconcile heartbeat"))]
    SendInitialReconcileHeartbeat,

    #[snafu(display("failed to patch CRD {crd_name:?}"))]
    PatchCrd {
        source: kube::Error,
        crd_name: String,
    },
}

/// Maintains various custom resource definitions.
///
/// When running this, the following operations are done:
///
/// - Apply the CRDs when starting up
/// - Reconcile the CRDs when the conversion webhook certificate is rotated
pub struct CustomResourceDefinitionMaintainer {
    client: Client,
    certificate_rx: mpsc::Receiver<Certificate>,

    definitions: Vec<CustomResourceDefinition>,
    options: CustomResourceDefinitionMaintainerOptions,

    initial_reconcile_tx: Option<oneshot::Sender<()>>,
}

impl CustomResourceDefinitionMaintainer {
    /// Creates and returns a new [`CustomResourceDefinitionMaintainer`] which manages one or more
    /// custom resource definitions.
    ///
    /// ## Parameters
    ///
    /// This function expects four parameters:
    ///
    /// - `client`: A [`Client`] to interact with the Kubernetes API server. It continuously patches
    ///   the CRDs when the TLS certificate is rotated.
    /// - `certificate_rx`: A [`mpsc::Receiver`] to receive newly generated TLS certificates. The
    ///   certificate data sent through the channel is used to set the caBundle in the conversion
    ///   section of the CRD.
    /// - `definitions`: An iterator of [`CustomResourceDefinition`]s which should be maintained
    ///   by this maintainer. If the iterator is empty, the maintainer returns early without doing
    ///   any work. As such, a polling mechanism which waits for all futures should be used to
    ///   prevent premature termination of the operator.
    /// - `options`: Provides [`CustomResourceDefinitionMaintainerOptions`] to customize various
    ///   parts of the maintainer. In the future, this will be converted to a builder, to enable a
    ///   cleaner API interface.
    ///
    /// ## Return Values
    ///
    /// This function returns a 2-tuple (pair) of values:
    ///
    /// - The [`CustomResourceDefinitionMaintainer`] itself. This is used to run the maintainer.
    ///   See [`CustomResourceDefinitionMaintainer::run`] for more details.
    /// - The [`oneshot::Receiver`] which will be used to send out a message once the initial
    ///   CRD reconciliation ran. This signal can be used to trigger the deployment of custom
    ///   resources defined by the maintained CRDs.
    pub fn new(
        client: Client,
        certificate_rx: mpsc::Receiver<Certificate>,
        definitions: impl IntoIterator<Item = CustomResourceDefinition>,
        options: CustomResourceDefinitionMaintainerOptions,
    ) -> (Self, oneshot::Receiver<()>) {
        let (initial_reconcile_tx, initial_reconcile_rx) = oneshot::channel();
        let initial_reconcile_tx = Some(initial_reconcile_tx);

        let maintainer = Self {
            definitions: definitions.into_iter().collect(),
            initial_reconcile_tx,
            certificate_rx,
            options,
            client,
        };

        (maintainer, initial_reconcile_rx)
    }

    /// Runs the [`CustomResourceDefinitionMaintainer`] asynchronously.
    ///
    /// This needs to be polled in parallel with other parts of an operator, like controllers or
    /// webhook servers. If it is disabled, the returned future immediately resolves to
    /// [`std::task::Poll::Ready`] and thus doesn't consume any resources.
    pub async fn run(mut self) -> Result<(), Error> {
        let CustomResourceDefinitionMaintainerOptions {
            operator_service_name,
            operator_namespace,
            field_manager,
            webhook_https_port: https_port,
            disabled,
        } = self.options;

        // If the maintainer is disabled or there are no custom resource definitions, immediately
        // return without doing any work.
        if disabled || self.definitions.is_empty() {
            return Ok(());
        }

        // This get's polled by the async runtime on a regular basis (or when woken up). Once we
        // receive a message containing the newly generated TLS certificate for the conversion
        // webhook, we need to update the caBundle in the CRD.
        while let Some(certificate) = self.certificate_rx.recv().await {
            tracing::info!(
                k8s.crd.names = ?self.definitions.iter().map(CustomResourceDefinition::name_any).collect::<Vec<_>>(),
                "reconciling custom resource definitions"
            );

            // The caBundle needs to be provided as a base64-encoded PEM envelope.
            let ca_bundle = certificate
                .to_pem(LineEnding::LF)
                .context(EncodeCertificateAuthorityAsPemSnafu)?;

            let crd_api: Api<CustomResourceDefinition> = Api::all(self.client.clone());

            for mut crd in self.definitions.iter().cloned() {
                let crd_kind = &crd.spec.names.kind;
                let crd_name = crd.name_any();

                tracing::debug!(
                    k8s.crd.kind = crd_kind,
                    k8s.crd.name = crd_name,
                    "reconciling custom resource definition"
                );

                crd.spec.conversion = Some(CustomResourceConversion {
                    strategy: "Webhook".to_owned(),
                    webhook: Some(WebhookConversion {
                        // conversionReviewVersions indicates what ConversionReview versions are
                        // supported by the webhook. The first version in the list understood by the
                        // API server is sent to the webhook. The webhook must respond with a
                        // ConversionReview object in the same version it received. We only support
                        // the stable v1 ConversionReview to keep the implementation as simple as
                        // possible.
                        conversion_review_versions: vec!["v1".to_owned()],
                        client_config: Some(WebhookClientConfig {
                            service: Some(ServiceReference {
                                name: operator_service_name.clone(),
                                namespace: operator_namespace.clone(),
                                path: Some(format!("/convert/{crd_name}")),
                                port: Some(https_port.into()),
                            }),
                            // Here, ByteString takes care of encoding the provided content as
                            // base64.
                            ca_bundle: Some(ByteString(ca_bundle.as_bytes().to_vec())),
                            url: None,
                        }),
                    }),
                });

                // Deploy the updated CRDs using a server-side apply.
                let patch = Patch::Apply(&crd);
                let patch_params = PatchParams::apply(&field_manager);
                crd_api
                    .patch(&crd_name, &patch_params, &patch)
                    .await
                    .with_context(|_| PatchCrdSnafu { crd_name })?;
            }

            // After the reconciliation of the CRDs, the initial reconcile heartbeat is sent out
            // via the oneshot channel. This channel can only be used exactly once. The sender's
            // send method consumes self, and as such, the sender is wrapped in an Option to be
            // able to call take to consume the inner value.
            if let Some(initial_reconcile_tx) = self.initial_reconcile_tx.take() {
                match initial_reconcile_tx.send(()) {
                    Ok(_) => {}
                    Err(_) => return SendInitialReconcileHeartbeatSnafu.fail(),
                }
            }
        }

        Ok(())
    }
}

// TODO (@Techassi): Make this a builder instead
/// This contains required options to customize a [`CustomResourceDefinitionMaintainer`].
pub struct CustomResourceDefinitionMaintainerOptions {
    /// The service name used by the operator/conversion webhook.
    operator_service_name: String,

    /// The namespace the operator/conversion webhook runs in.
    operator_namespace: String,

    /// The name of the field manager used for the server-side apply.
    field_manager: String,

    /// The HTTPS port the conversion webhook listens on.
    webhook_https_port: u16,

    /// Indicates if the maintainer should be disabled.
    disabled: bool,
}
