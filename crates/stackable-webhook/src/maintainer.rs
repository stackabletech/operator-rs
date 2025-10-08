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
use snafu::{ResultExt, Snafu, ensure};
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
pub struct CustomResourceDefinitionMaintainer<'a> {
    client: Client,
    certificate_rx: mpsc::Receiver<Certificate>,

    definitions: Vec<CustomResourceDefinition>,
    options: CustomResourceDefinitionMaintainerOptions<'a>,

    initial_reconcile_tx: oneshot::Sender<()>,
}

impl<'a> CustomResourceDefinitionMaintainer<'a> {
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
    ///
    /// ## Example
    ///
    /// ```no_run
    /// # use stackable_operator::crd::s3::{S3Connection, S3ConnectionVersion, S3Bucket, S3BucketVersion};
    /// # use tokio::sync::mpsc::channel;
    /// # use x509_cert::Certificate;
    /// # use kube::Client;
    /// use stackable_webhook::maintainer::{
    ///     CustomResourceDefinitionMaintainerOptions,
    ///     CustomResourceDefinitionMaintainer,
    /// };
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let (certificate_tx, certificate_rx) = channel(1);
    /// let options = CustomResourceDefinitionMaintainerOptions {
    ///     operator_name: "my-service-name",
    ///     operator_namespace: "my-namespace",
    ///     webhook_https_port: 8443,
    ///     disabled: true,
    /// };
    ///
    /// let client = Client::try_default().await.unwrap();
    ///
    /// let definitions = vec![
    ///     S3Connection::merged_crd(S3ConnectionVersion::V1Alpha1).unwrap(),
    ///     S3Bucket::merged_crd(S3BucketVersion::V1Alpha1).unwrap(),
    /// ];
    ///
    /// let (maintainer, initial_reconcile_rx) = CustomResourceDefinitionMaintainer::new(
    ///     client,
    ///     certificate_rx,
    ///     definitions,
    ///     options,
    /// );
    /// # }
    /// ```
    pub fn new(
        client: Client,
        certificate_rx: mpsc::Receiver<Certificate>,
        definitions: impl IntoIterator<Item = CustomResourceDefinition>,
        options: CustomResourceDefinitionMaintainerOptions<'a>,
    ) -> (Self, oneshot::Receiver<()>) {
        let (initial_reconcile_tx, initial_reconcile_rx) = oneshot::channel();

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
            operator_namespace,
            webhook_https_port,
            operator_name,
            disabled,
        } = self.options;

        // If the maintainer is disabled or there are no custom resource definitions, immediately
        // return without doing any work.
        if disabled || self.definitions.is_empty() {
            return Ok(());
        }

        // This channel can only be used exactly once. The sender's send method consumes self, and
        // as such, the sender is wrapped in an Option to be able to call take to consume the inner
        // value.
        let mut initial_reconcile_tx = Some(self.initial_reconcile_tx);

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

            for crd in self.definitions.iter_mut() {
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
                                name: operator_name.to_owned(),
                                namespace: operator_namespace.to_owned(),
                                path: Some(format!("/convert/{crd_name}")),
                                port: Some(webhook_https_port.into()),
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
                let patch_params = PatchParams::apply(operator_name);
                crd_api
                    .patch(&crd_name, &patch_params, &patch)
                    .await
                    .with_context(|_| PatchCrdSnafu { crd_name })?;
            }

            // After the reconciliation of the CRDs, the initial reconcile heartbeat is sent out
            // via the oneshot channel.
            if let Some(initial_reconcile_tx) = initial_reconcile_tx.take() {
                ensure!(
                    initial_reconcile_tx.send(()).is_ok(),
                    SendInitialReconcileHeartbeatSnafu
                );
            }
        }

        Ok(())
    }
}

// TODO (@Techassi): Make this a builder instead
/// This contains required options to customize a [`CustomResourceDefinitionMaintainer`].
pub struct CustomResourceDefinitionMaintainerOptions<'a> {
    /// The service name used by the operator/conversion webhook and as a field manager.
    pub operator_name: &'a str,

    /// The namespace the operator/conversion webhook runs in.
    pub operator_namespace: &'a str,

    /// The HTTPS port the conversion webhook listens on.
    pub webhook_https_port: u16,

    /// Indicates if the maintainer should be disabled.
    pub disabled: bool,
}
