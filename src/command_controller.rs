//! Generic controller to add command CRDs for restart, start, stop etc. operations as
//! specified in [Stackable ADR010](https://github.com/stackabletech/documentation/blob/main/adr/ADR010-command_pattern.adoc).
//!
//! # Example
//!
//! ```no_run
//! use stackable_operator::Crd;
//! use stackable_operator::{error, client};
//!
//! #[derive(Clone, CustomResource, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
//! #[kube(
//!     group = "command.foo.stackable.tech",
//!     version = "v1",
//!     kind = "Restart",
//!     namespaced
//! )]
//! #[serde(rename_all = "camelCase")]
//! pub struct FooCommandRestartSpec {
//!     pub name: String,
//! }
//!
//! impl stackable_operator::command_controller::CommandCrd for Restart {
//!     type Parent = Foo;
//!     fn get_name(&self) -> String {
//!         self.spec.name.clone()
//!     }
//! }
//!
//! impl Crd for Restart {
//!     const RESOURCE_NAME: &'static str = "restarts.command.foo.stackable.tech";
//!     const CRD_DEFINITION: &'static str = "
//! apiVersion: apiextensions.k8s.io/v1
//! kind: CustomResourceDefinition
//! metadata:
//!   name: restarts.command.foo.stackable.tech
//! spec:
//!   group: command.foo.stackable.tech
//!   names:
//!     kind: Restart
//!     singular: restart
//!     plural: restarts
//!     listKind: RestartList
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
//!                   type: string";
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), error::Error> {
//!    stackable_operator::logging::initialize_logging("FOO_OPERATOR_LOG");
//!    let client = client::create_client(Some("foo.stackable.tech".to_string())).await?;
//!
//!    stackable_operator::crd::ensure_crd_created::<Foo>(client.clone()).await?;
//!    stackable_operator::crd::ensure_crd_created::<Restart>(client.clone()).await?;
//!
//!    tokio::join!(
//!        stackable_foo_operator::create_controller(client.clone()),
//!        stackable_operator::command_controller::create_command_controller::<Restart>(client)
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
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

const FINALIZER_NAME: &str = "command.stackable.tech/cleanup";

type CommandReconcileResult = ReconcileResult<Error>;

/// The CommandCrd trait expects a get_name() method which should return the specified field in the
/// CustomResource that will later be matched on metadata.name to find the "parent" resource.
/// The Parent is the watched resource of the standard reconcile controller of which we want to set
/// the OwnerReference to our command resource.
pub trait CommandCrd: Meta + Clone + DeserializeOwned + Serialize + Debug + Send + Sync {
    type Parent: Meta + Clone + DeserializeOwned + Debug + Send + Sync;
    fn get_name(&self) -> String;
}

struct CommandState<T: CommandCrd>
where
    T: CommandCrd,
{
    context: ReconciliationContext<T>,
}

impl<T> CommandState<T>
where
    T: CommandCrd,
{
    /// This controller only sets the parent owner reference to our custom resource object.
    /// Later in the main controller loop we can list all references and decide how to act
    /// on different commands.
    async fn set_owner_reference(&mut self) -> CommandReconcileResult {
        let owner = find_owner::<T::Parent>(
            &self.context.client.clone(),
            &self.context.resource.get_name(),
        )
        .await?;

        let owner_reference =
            metadata::object_to_owner_reference::<T::Parent>(owner.meta().clone(), true)?;

        patch_owner_reference(
            &self.context.client,
            &self.context.resource,
            &owner_reference,
        )
        .await?;

        Ok(ReconcileFunctionAction::Done)
    }
}

impl<T> ReconciliationState for CommandState<T>
where
    T: CommandCrd,
{
    type Error = Error;

    fn reconcile(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ReconcileFunctionAction, Self::Error>> + Send + '_>>
    {
        Box::pin(async move { self.set_owner_reference().await })
    }
}

#[derive(Debug)]
struct CommandStrategy<T: CommandCrd> {
    // TODO: Better workaround for PhantomData?
    // We use it here cause we need to make CommandStrategy generic to be able to do:
    // impl<T> ControllerStrategy for CommandStrategy<T>
    _ignore: Option<std::marker::PhantomData<T>>,
}

impl<T: CommandCrd> CommandStrategy<T> {
    pub fn new() -> CommandStrategy<T> {
        CommandStrategy { _ignore: None }
    }
}

#[async_trait]
impl<T> ControllerStrategy for CommandStrategy<T>
where
    T: CommandCrd,
{
    type Item = T;
    type State = CommandState<T>;
    type Error = Error;

    fn finalizer_name(&self) -> String {
        FINALIZER_NAME.to_string()
    }

    async fn init_reconcile_state(
        &self,
        context: ReconciliationContext<Self::Item>,
    ) -> Result<Self::State, Error> {
        Ok(CommandState { context })
    }
}

/// This creates an instance of a [`Controller`] which waits for incoming events and reconciles them.
///
/// This is an async method and the returned future needs to be consumed to make progress.
pub async fn create_command_controller<T>(client: Client)
where
    T: CommandCrd + 'static,
{
    let command_api: Api<T> = client.get_all_api();
    let parent_api: Api<T::Parent> = client.get_all_api();

    let controller = Controller::new(command_api).owns(parent_api, ListParams::default());

    let strategy = CommandStrategy::new();

    controller
        .run(client, strategy, Duration::from_secs(10))
        .await;
}

/// Find a Resource according to the metadata.name field.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `metadata_name` - The metadata.name value to be matched
///
async fn find_owner<T>(client: &Client, metadata_name: &str) -> OperatorResult<T>
where
    T: Clone + DeserializeOwned + Meta,
{
    let lp = ListParams::default().fields(&format!("metadata.name={}", metadata_name));
    let mut owners: Vec<T> = client.list(None, &lp).await?;

    if owners.is_empty() {
        return Err(Error::MissingCustomResource {
            name: metadata_name.to_string(),
        });
    }

    Ok(owners.pop().unwrap())
}

/// Add an OwnerReference to an existing Resource via merge strategy.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `resource` - The resource where to set the OwnerReference
/// * `owner_reference` - The OwnerReference to add
///
async fn patch_owner_reference<T>(
    client: &Client,
    resource: &T,
    owner_reference: &OwnerReference,
) -> OperatorResult<T>
where
    T: Clone + DeserializeOwned + Meta,
{
    // TODO: Check for existing owner references. As of now we just override.
    let new_metadata = serde_json::json!({
        "metadata": {
            "ownerReferences": [owner_reference]
        }
    });

    client.merge_patch(resource, new_metadata).await
}

/// Get a list of available commands of type T.
///
/// # Arguments
/// * `client` - Kubernetes client
///
pub async fn list_commands<T>(client: &Client) -> OperatorResult<Vec<T>>
where
    T: CommandCrd,
{
    let restart: Api<T> = client.get_all_api();
    let list = restart
        .list(&ListParams {
            ..ListParams::default()
        })
        .await?;

    Ok(list.items.to_vec())
}
