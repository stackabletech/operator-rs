//! Generic controller to add command CRDs for restart, start, stop (...) command operations as
//! specified in [Stackable ADR010](https://github.com/stackabletech/documentation/blob/main/adr/ADR010-command_pattern.adoc).
use crate::client::Client;
use crate::controller::{Controller, ControllerStrategy, ReconciliationState};
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

// TODO: remove after hackathon merge
const FINALIZER_NAME: &str = "command.stackable.tech/cleanup";

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
        if let Some(owner_references) = &self.context.resource.meta().owner_references {
            for owner_reference in owner_references {
                if owner_reference.name == self.context.resource.get_owner_name()
                    && owner_reference.kind == O::KIND
                {
                    return Ok(ReconcileFunctionAction::Done);
                }
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
                self.context.resource.namespace(),
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
        .list(&ListParams {
            ..ListParams::default()
        })
        .await?
        .items
        .to_vec();

    Ok(list)
}
