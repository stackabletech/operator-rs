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

    pub async fn run(mut self) -> Result<(), Error> {
        let CustomResourceDefinitionMaintainerOptions {
            operator_service_name,
            operator_namespace,
            field_manager,
            https_port,
            disabled,
        } = self.options;

        // If the maintainer is disabled, immediately return without doing any work.
        if disabled {
            return Ok(());
        }

        while let Some(certificate) = self.certificate_rx.recv().await {
            tracing::info!(
                k8s.crd.names = ?self.definitions.iter().map(CustomResourceDefinition::name_any).collect::<Vec<_>>(),
                "reconciling custom resource definitions"
            );

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
                        // conversionReviewVersions indicates what ConversionReview versions are understood/preferred by the webhook.
                        // The first version in the list understood by the API server is sent to the webhook.
                        // The webhook must respond with a ConversionReview object in the same version it received.
                        conversion_review_versions: vec!["v1".to_owned()],
                        client_config: Some(WebhookClientConfig {
                            service: Some(ServiceReference {
                                name: operator_service_name.clone(),
                                namespace: operator_namespace.clone(),
                                path: Some(format!("/convert/{crd_name}")),
                                port: Some(https_port.into()),
                            }),
                            ca_bundle: Some(ByteString(ca_bundle.as_bytes().to_vec())),
                            url: None,
                        }),
                    }),
                });

                let patch = Patch::Apply(&crd);
                let patch_params = PatchParams::apply(&field_manager);
                crd_api
                    .patch(&crd_name, &patch_params, &patch)
                    .await
                    .with_context(|_| PatchCrdSnafu { crd_name })?;
            }

            // Once all CRDs are reconciled, send a heartbeat for consumers to be notified that
            // custom resources of these kinds can bow be deployed.
            if let Some(initial_reconcile_tx) = self.initial_reconcile_tx.take() {
                initial_reconcile_tx
                    .send(())
                    .ignore_context(SendInitialReconcileHeartbeatSnafu)?
            }
        }

        Ok(())
    }
}

// TODO (@Techassi): Make this a builder instead
pub struct CustomResourceDefinitionMaintainerOptions {
    operator_service_name: String,
    operator_namespace: String,
    field_manager: String,
    https_port: u16,
    disabled: bool,
}

trait ResultContextExt<T> {
    fn ignore_context<C, E2>(self, context: C) -> Result<T, E2>
    where
        C: snafu::IntoError<E2, Source = snafu::NoneError>,
        E2: std::error::Error + snafu::ErrorCompat;
}

impl<T, E> ResultContextExt<T> for Result<T, E> {
    fn ignore_context<C, E2>(self, context: C) -> Result<T, E2>
    where
        C: snafu::IntoError<E2, Source = snafu::NoneError>,
        E2: std::error::Error + snafu::ErrorCompat,
    {
        match self {
            Ok(v) => Ok(v),
            Err(_) => Err(context.into_error(snafu::NoneError)),
        }
    }
}
