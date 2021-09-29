use crate::client::Client;
use crate::command_controller::Command;
use crate::error::Error::ConversionError;
use crate::error::OperatorResult;
use crate::status::HasCurrentCommand;
use crate::CustomResourceExt;
use json_patch::{PatchOperation, RemoveOperation};
use k8s_openapi::serde::de::DeserializeOwned;
use kube::api::{ApiResource, DynamicObject, ListParams, Resource};
use kube::core::object::HasStatus;
use kube::Api;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use tracing::{debug, error, trace};

/// Retrieve a timestamp in format: "2021-03-23T16:20:19Z".
/// Required to set command start and finish timestamps.
pub fn get_current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Implemented on a cluster object this can be called to retrieve the order that roles need to
/// be restarted
pub trait HasRoleRestartOrder {
    fn get_role_restart_order() -> Vec<String>;
}

/// Implemented on a Cluster object, this can be used to retrieve the types of commands that
/// can be used to administer this cluster.
pub trait HasCommands {
    fn get_command_types() -> Vec<ApiResource>;
}

/// When implemented this command can be executed in a rolling fashion
pub trait CanBeRolling: Command {
    fn is_rolling(&self) -> bool;
}

/// When implemented, this command can be restricted to only run on a subset of roles that are
/// available for this application
pub trait HasRoles: Command {
    fn get_role_order(&self) -> Option<Vec<String>>;
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRef {
    pub uid: String,
    pub name: String,
    pub namespace: String,
    pub kind: String,
}

impl TryFrom<DynamicObject> for CommandRef {
    type Error = crate::error::Error;

    fn try_from(command: DynamicObject) -> Result<Self, Self::Error> {
        let report_error = |field: &str| ConversionError {
            message: format!(
                "Error converting to [CommandRef] from [DynamicObject]: [{}] cannot be empty!",
                field
            ),
        };

        Ok(CommandRef {
            uid: command.metadata.uid.ok_or_else(|| report_error("uid"))?,
            name: command.metadata.name.ok_or_else(|| report_error("name"))?,
            namespace: command
                .metadata
                .namespace
                .ok_or_else(|| report_error("namespace"))?,
            kind: command.types.ok_or_else(|| report_error("kind"))?.kind,
        })
    }
}

pub async fn clear_current_command<T>(client: &Client, resource: &mut T) -> OperatorResult<bool>
where
    T: CustomResourceExt + Resource + Clone + Debug + DeserializeOwned + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand + Debug + Default + Serialize,
    <T as Resource>::DynamicType: Default,
{
    let tracking_location = <<T as HasStatus>::Status as HasCurrentCommand>::tracking_location();

    let patch = json_patch::Patch(vec![PatchOperation::Remove(RemoveOperation {
        path: String::from(tracking_location),
    })]);

    trace!(
        "Patching current command at location {}: {:?}",
        tracking_location,
        patch
    );

    client.json_patch_status(resource, patch).await?;

    // Now we need to clear the command in the stashed status in our context
    // In theory this can create an inconsistent state if the operator crashes after
    // patching the status above and performing this `clear`. However, as this would in
    // effect just mean restarting the reconciliation which would then read the changed
    // status from Kubernetes and thus have a clean state again this seems acceptable
    if let Some(status) = resource.status_mut() {
        status.clear_current_command();
    }

    Ok(true)
}

pub async fn maybe_update_current_command<T>(
    client: &Client,
    resource: &mut T,
    command: &CommandRef,
) -> OperatorResult<()>
where
    T: CustomResourceExt + Resource + Clone + Debug + DeserializeOwned + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand + Debug + Default + Serialize,
    <T as Resource>::DynamicType: Default,
{
    let resource_cloned = resource.clone();
    let status = resource.status_mut().get_or_insert_with(Default::default);

    if status
        .current_command()
        .filter(|cmd| *cmd != *command)
        .is_none()
    {
        // Current command is none or not equal to the new command -> we need to patch
        status.set_current_command(command.clone());

        debug!("Setting currentCommand to [{:?}]", command);

        client.merge_patch_status(&resource_cloned, &status).await?;
    }

    Ok(())
}

/// Return the current command in the `resource` status or an available command.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `resource` - Cluster custom resource
/// * `resources` - Command API resources
///
pub async fn current_command<T>(
    client: &Client,
    resource: &T,
    resources: &[ApiResource],
) -> OperatorResult<Option<CommandRef>>
where
    T: Resource + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand,
{
    match resource.status() {
        Some(status) if status.current_command().is_some() => {
            debug!(
                "Found current command in status: [{:?}]",
                status.current_command()
            );
            Ok(status.current_command())
        }
        Some(_) => {
            debug!("No current command set in status, retrieving from k8s...");
            get_next_command(client, resources).await
        }
        None => {
            debug!("No status set, retrieving command from k8s...");
            get_next_command(client, resources).await
        }
    }
}

/// Tries to retrieve the Command object for a provided [`CommandRef`].
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `command_ref` - `CommandRef` referencing the actual command
///
pub async fn materialize_command<T>(client: &Client, command_ref: &CommandRef) -> OperatorResult<T>
where
    T: Resource + Clone + Debug + DeserializeOwned,
    <T as Resource>::DynamicType: Default,
{
    client
        .get(&command_ref.name, Some(&command_ref.namespace))
        .await
}

/// Collect and sort all available commands and return the first (the one with
/// the oldest creation timestamp) element.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `resources` - Kubernetes API Resources
///
pub async fn get_next_command(
    client: &Client,
    resources: &[ApiResource],
) -> OperatorResult<Option<CommandRef>> {
    let mut all_commands = collect_commands(client, resources).await?;
    all_commands.sort_by(|a, b| {
        a.metadata
            .creation_timestamp
            .cmp(&b.metadata.creation_timestamp)
    });

    trace!("Commands found: {:?}", all_commands);

    match all_commands
        .into_iter()
        // TODO: We need to traitify this in order to remove the hardcoded part to filter
        //   finished commands (those that have `finished_at` set)
        .filter(|cmd| {
            cmd.data
                .get("status")
                .and_then(|spec| spec.get("finishedAt"))
                .is_none()
        })
        .map(|command| command.try_into())
        .into_iter()
        .collect::<OperatorResult<Vec<CommandRef>>>()
    {
        Ok(mut commands) => {
            debug!("Commands to be processed: {:?}", commands);
            Ok(commands.pop())
        }
        Err(err) => {
            error!(
                "Error converting at least one existing command to a working object: {:?}",
                err
            );
            Err(err)
        }
    }
}

/// Collect all provided api resources and return them in one big list.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `resources` - Kubernetes API Resources
///
async fn collect_commands(
    client: &Client,
    resources: &[ApiResource],
) -> OperatorResult<Vec<DynamicObject>> {
    let mut all_commands = vec![];
    for resource in resources {
        all_commands.append(&mut list_resources(client, resource).await?);
    }
    Ok(all_commands)
}

/// Collect all objects of a api resource and return them in a list.
///
/// # Arguments
/// * `client` - Kubernetes client
/// * `resource` - Kubernetes API Resource
///
pub async fn list_resources(
    client: &Client,
    resource: &ApiResource,
) -> OperatorResult<Vec<DynamicObject>> {
    let kube_client = client.as_kube_client();
    let api: Api<DynamicObject> = Api::all_with(kube_client, resource);
    Ok(api.list(&ListParams::default()).await?.items)
}
