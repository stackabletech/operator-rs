//! Generic controller to add command CRDs for restart, start, stop (...) command operations as
//! specified in [Stackable ADR010](https://github.com/stackabletech/documentation/blob/main/adr/ADR010-command_pattern.adoc).
//!
//! # Example
//!
//! ```no_run
//! use kube::CustomResource;
//! use kube::api::Meta;
//! use stackable_operator::Crd;
//! use stackable_operator::{client, error};
//! use stackable_operator::client::Client;
//! use stackable_operator::error::Error;
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
//! #[kube(
//!     group = "foo.stackable.tech",
//!     version = "v1",
//!     kind = "FooCluster",
//!     namespaced
//! )]
//! #[kube(status = "FooClusterStatus")]
//! #[serde(rename_all = "camelCase")]
//! pub struct FooClusterSpec {
//!     pub name: String,
//! }
//!
//! #[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
//! #[serde(rename_all = "camelCase")]
//! pub struct FooClusterStatus {}
//!
//! impl Crd for FooCluster {
//!     const RESOURCE_NAME: &'static str = "fooclusters.foo.stackable.tech";
//!     const CRD_DEFINITION: &'static str = "...";
//! }
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
//!
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
//!
//! #[tokio::main]
//! async fn main() -> Result<(),Error> {
//!    stackable_operator::logging::initialize_logging("FOO_OPERATOR_LOG");
//!    let client = client::create_client(Some("foo.stackable.tech".to_string())).await?;
//!
//!    stackable_operator::crd::ensure_crd_created::<FooCluster>(client.clone()).await?;
//!    stackable_operator::crd::ensure_crd_created::<Bar>(client.clone()).await?;
//!
//!    tokio::join!(
//!        // create main custom resource controller like ...
//!        // stackable_foocluster_operator.create_controller(client.clone());
//!        // create command controller
//!        stackable_operator::command_controller::create_command_controller::<Bar, FooCluster>(client)
//!    );
//!    Ok(())
//! }
//! ```
//!
use crate::client::Client;
use crate::controller::{Controller, ControllerStrategy, ReconciliationState};
use crate::controller_ref::get_controller_of;
use crate::error::{Error, OperatorResult};
use crate::metadata;
use crate::reconcile::{ReconcileFunctionAction, ReconcileResult, ReconciliationContext};
use async_trait::async_trait;
use json_patch::{AddOperation, PatchOperation};
use kube::api::{ListParams, Meta};
use kube::Api;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tracing::trace;

/// Trait for all commands to be implemented. We need to retrieve the name of the
/// main controller custom resource.
/// The referenced resource has to be in the same namespace as the command itself.
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
}

impl<C, O> CommandState<C, O>
where
    C: Command + Clone + DeserializeOwned + Meta,
    O: Clone + DeserializeOwned + Meta,
{
    /// Check if our custom resource command already has the owner reference set to the main
    /// controller custom resource. If so we can stop the reconcile.
    async fn owner_reference_existing(&mut self) -> ReconcileResult<Error> {
        // If owner_references exist, check if any match our main resource owner reference.
        if let Some(owner_reference) = get_controller_of(&self.context.resource) {
            if owner_reference.name == self.context.resource.get_owner_name()
                && owner_reference.kind == O::KIND
            {
                return Ok(ReconcileFunctionAction::Done);
            }
        }

        Ok(ReconcileFunctionAction::Continue)
    }

    /// Try to retrieve the owner (main controller custom resource).
    /// This is required to build the owner reference for our custom resource.
    async fn get_owner(&mut self) -> ReconcileResult<Error> {
        let owner: O = self
            .context
            .client
            .get(
                &self.context.resource.get_owner_name(),
                self.context.resource.namespace().as_deref(),
            )
            .await?;

        trace!(
            "Found owner [{}] for command [{}]",
            &self.context.resource.get_owner_name(),
            &self.context.resource.name()
        );

        self.owner = Some(owner);

        Ok(ReconcileFunctionAction::Continue)
    }

    /// If the owner (main controller custom resource), we set its owner reference
    /// to our command custom resource.
    async fn set_owner_reference(&self) -> ReconcileResult<Error> {
        let owner_reference = metadata::object_to_owner_reference::<O>(
            self.owner.as_ref().unwrap().meta().clone(),
            true,
        )?;

        let owner_references_path = "/metadata/ownerReferences".to_string();
        // we do not need to test here, if the owner ref is already in here, we would
        // not reach this point in the reconcile loop (-> check owner_reference_existing())
        let patch = json_patch::Patch(vec![PatchOperation::Add(AddOperation {
            path: owner_references_path,
            value: serde_json::json!([owner_reference]),
        })]);

        self.context
            .client
            .json_patch(&self.context.resource, patch)
            .await?;

        Ok(ReconcileFunctionAction::Continue)
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
            self.owner_reference_existing()
                .await?
                .then(self.get_owner())
                .await?
                .then(self.set_owner_reference())
                .await
        })
    }
}

#[derive(Debug)]
struct CommandStrategy<C, O> {
    // TODO: Better workaround for PhantomData?
    // We use it here because we need to make CommandStrategy generic to be able to do:
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

    async fn init_reconcile_state(
        &self,
        context: ReconciliationContext<Self::Item>,
    ) -> Result<Self::State, Error> {
        Ok(CommandState {
            context,
            owner: None,
        })
    }
}

/// This creates an instance of a [`Controller`] which waits for incoming commands.
/// For each command, we try to find the referenced resource and will set the Owner Reference
/// of the command to this referenced resource.
/// If we can't find the referenced object we TODO: delete?.
/// This means that the controller of the parent resource can now watch for commands and this
/// helper controller will make sure that they trigger a reconcile for the parent by setting the OwnerReference.
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
        .list(&ListParams::default())
        .await?
        .items
        .to_vec();

    Ok(list)
}
