//! Generic controller to add command CRDs for restart, start, stop (...) command operations as
//! specified in [Stackable ADR010](https://github.com/stackabletech/documentation/blob/main/adr/ADR010-command_pattern.adoc).
//!
//! # Example
//!
//! ```no_run
//! use kube::api::Meta;
//! use stackable_operator::Crd;
//! use stackable_operator::{error, client};
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
//! #[kube(
//!     group = "command.foo.stackable.tech",
//!     version = "v1",
//!     kind = "Bar",
//!     namespaced
//! )]
//! #[kube(status = "BarCommandStatus")]
//! #[serde(rename_all = "camelCase")]
//! pub struct BarCommandSpec {
//!     pub name: String,
//! }
//! #[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
//! #[serde(rename_all = "camelCase")]
//! pub struct BarCommandStatus {}
//!
//! impl stackable_operator::command_controller::Command for Bar {
//!     fn get_owner_name(&self) -> String {
//!         self.spec.name.clone()
//!     }
//! }
//!
//! impl Crd for Bar {
//!     const RESOURCE_NAME: &'static str = "bars.command.foo.stackable.tech";
//!     const CRD_DEFINITION: &'static str = "
//! apiVersion: apiextensions.k8s.io/v1
//! kind: CustomResourceDefinition
//! metadata:
//!   name: bars.command.foo.stackable.tech
//! spec:
//!   group: command.foo.stackable.tech
//!   names:
//!     kind: Bar
//!     singular: bar
//!     plural: bars
//!     listKind: BarList
//!   scope: Namespaced
//!   versions:
//!     - name: v1
//!       served: true
//!       storage: true
//!       schema:
//!         openAPIV3Schema:
//!           type: object
//!           properties:
//!             spec:
//!               type: object
//!               properties:
//!                 name:
//!                   type: string
//!             status:
//!               nullable: true
//!               type: object
//!               properties:
//!                 startedAt:
//!                   type: string
//!                 finishedAt:
//!                   type: string
//!                 message:
//!                   type: string
//!       subresources:
//!         status: {}";
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), error::Error> {
//!    stackable_operator::logging::initialize_logging("FOO_OPERATOR_LOG");
//!    let client = client::create_client(Some("foo.stackable.tech".to_string())).await?;
//!
//!    stackable_operator::crd::ensure_crd_created::<Foo>(client.clone()).await?;
//!    stackable_operator::crd::ensure_crd_created::<Bar>(client.clone()).await?;
//!
//!    tokio::join!(
//!        stackable_foo_operator::create_controller(client.clone()),
//!        stackable_operator::command_controller::create_command_controller::<Bar,Foo>(client)
//!    );
//!    Ok(())
//! }
//! ```
//!
use crate::client::Client;
use crate::controller::{Controller, ControllerStrategy, ReconciliationState};
use crate::error::{Error, OperatorResult};
use crate::metadata;
use crate::reconcile::{ReconcileFunctionAction, ReconcileResult, ReconciliationContext};
use async_trait::async_trait;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::{ListParams, Meta};
use kube::Api;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

// TODO: remove after hackathon merge
const FINALIZER_NAME: &str = "command.stackable.tech/cleanup";

/// Trait for all commands to be implemented. We need to retrieve the name of the
/// main controller custom resource.
pub trait Command {
    /// Retrieve the potential "Owner" name of this custom resource
    fn get_owner_name(&self) -> String;
}

struct CommandState<C, O>
where
    C: Command + Clone + DeserializeOwned + Meta,
    O: Clone + DeserializeOwned + Meta,
{
    context: ReconciliationContext<C>,
    owner: Option<O>,
    owner_reference: Option<OwnerReference>,
}

impl<C, O> CommandState<C, O>
where
    C: Command + Clone + DeserializeOwned + Meta,
    O: Clone + DeserializeOwned + Meta,
{
    /// Try to retrieve the owner (main controller custom resource).
    /// This is required to build the owner reference for our custom resource.
    async fn get_owner(&mut self) -> ReconcileResult<Error> {
        // find main controller custom resource
        self.owner = find_owner::<O>(
            &self.context.client.clone(),
            &self.context.resource.get_owner_name(),
            &self.context.resource.namespace(),
        )
        .await?;

        Ok(ReconcileFunctionAction::Continue)
    }

    /// Check if our custom resource already has the owner reference on the main
    /// controller custom resource set.
    async fn check_owner_reference(&mut self) -> ReconcileResult<Error> {
        // store owner reference
        self.owner_reference = Some(metadata::object_to_owner_reference::<O>(
            self.owner.as_ref().unwrap().meta().clone(),
            true,
        )?);

        // If owner_references exist, check if any match our main resource owner reference.
        if let Some(owner_references) = &self.context.resource.meta().owner_references {
            if self.owner_reference.is_none() {
                return Err(Error::MissingOwnerReference {
                    command: Meta::name(&self.context.resource),
                    owner: self.context.resource.get_owner_name(),
                });
            }

            // Already set -> we are done
            if owner_references.contains(&self.owner_reference.as_ref().unwrap()) {
                return Ok(ReconcileFunctionAction::Done);
            }
        }

        Ok(ReconcileFunctionAction::Continue)
    }

    /// If the owner (main controller custom resource), we set its owner reference
    /// to our command custom resource.
    async fn set_owner_reference(&self) -> ReconcileResult<Error> {
        patch_owner_reference(
            &self.context.client,
            &self.context.resource,
            &self.owner_reference,
        )
        .await?;

        Ok(ReconcileFunctionAction::Done)
    }
}

impl<C, O> ReconciliationState for CommandState<C, O>
where
    C: Command + Clone + DeserializeOwned + Meta + Send + Sync,
    O: Clone + DeserializeOwned + Meta + Send + Sync,
{
    type Error = Error;

    fn reconcile(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + Send + '_>>
    {
        Box::pin(async move {
            self.get_owner()
                .await?
                .then(self.check_owner_reference())
                .await?
                .then(self.set_owner_reference())
                .await
        })
    }
}

#[derive(Debug)]
struct CommandStrategy<C, O> {
    // TODO: Better workaround for PhantomData?
    // We use it here cause we need to make CommandStrategy generic to be able to do:
    // impl<C,O> ControllerStrategy for CommandStrategy<C,O>
    _ignore: Option<std::marker::PhantomData<C>>,
    _ignore2: Option<std::marker::PhantomData<O>>,
}

impl<C, O> CommandStrategy<C, O> {
    pub fn new() -> CommandStrategy<C, O> {
        CommandStrategy {
            _ignore: None,
            _ignore2: None,
        }
    }
}

#[async_trait]
impl<C, O> ControllerStrategy for CommandStrategy<C, O>
where
    C: Command + Clone + DeserializeOwned + Meta + Send + Sync,
    O: Clone + DeserializeOwned + Meta + Send + Sync,
{
    type Item = C;
    type State = CommandState<C, O>;
    type Error = Error;

    // TODO: remove after Hackathon merge
    fn finalizer_name(&self) -> String {
        FINALIZER_NAME.to_string()
    }

    async fn init_reconcile_state(
        &self,
        context: ReconciliationContext<Self::Item>,
    ) -> Result<Self::State, Error> {
        Ok(CommandState {
            context,
            owner: None,
            owner_reference: None,
        })
    }
}

/// This creates an instance of a [`Controller`] which waits for incoming commands.
/// For each command, we set the owner reference of our main controller custom resource, to
/// list and process commands in the main reconcile loop.
///
/// This is an async method and the returned future needs to be consumed to make progress.
pub async fn create_command_controller<C, O>(client: Client)
where
    C: Command + Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
    O: Clone + Debug + DeserializeOwned + Meta + Send + Sync + 'static,
{
    let command_api: Api<C> = client.get_all_api();

    let controller = Controller::new(command_api);

    let strategy = CommandStrategy::<C, O>::new();

    controller
        .run(client, strategy, Duration::from_secs(10))
        .await;
}

/// Find a resource according to the metadata.name field.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `metadata_name` - The metadata.name value to be matched
///
async fn find_owner<O>(
    client: &Client,
    metadata_name: &str,
    namespace: &Option<String>,
) -> OperatorResult<Option<O>>
where
    O: Clone + DeserializeOwned + Meta,
{
    let lp = ListParams::default().fields(&format!("metadata.name={}", metadata_name));
    let mut owners: Vec<O> = client.list(namespace.clone(), &lp).await?;

    // TODO: What to do with commands that have no existing owner?
    if owners.is_empty() {
        return Err(Error::MissingCustomResource {
            name: metadata_name.to_string(),
        });
    }

    Ok(owners.pop())
}

/// Add an OwnerReference to an existing resource via merge strategy.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `resource` - The resource where to set the OwnerReference
/// * `owner_reference` - The OwnerReference to add
///
async fn patch_owner_reference<C>(
    client: &Client,
    resource: &C,
    owner_reference: &Option<OwnerReference>,
) -> OperatorResult<()>
where
    C: Clone + DeserializeOwned + Meta,
{
    let mut owner_references = vec![];

    if let Some(references) = &mut resource.meta().owner_references.clone() {
        owner_references.append(references);
    }

    if let Some(owner_ref) = owner_reference {
        owner_references.push(owner_ref.clone());

        let new_metadata = serde_json::json!({
            "metadata": {
                "ownerReferences": owner_references
            }
        });

        client.merge_patch(resource, new_metadata).await?;
    }

    Ok(())
}

/// Get a list of available commands of custom resource T.
///
/// # Arguments
/// * `client` - Kubernetes client
///
pub async fn list_commands<C>(client: &Client) -> OperatorResult<Vec<C>>
where
    C: Meta + Clone + DeserializeOwned,
{
    let command_api: Api<C> = client.get_all_api();
    let list = command_api
        .list(&ListParams {
            ..ListParams::default()
        })
        .await?
        .items
        .to_vec();

    Ok(list)
}
