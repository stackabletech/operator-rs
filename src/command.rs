use crate::client::Client;
use crate::command_controller::Command;
use crate::error::Error::ConversionError;
use crate::error::OperatorResult;
use crate::status::HasCurrentCommand;
use crate::CustomResourceExt;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::api::{ApiResource, DynamicObject, ListParams, Resource};
use kube::core::object::HasStatus;
use kube::Api;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use tracing::{info, warn};

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
    pub command_uid: String,
    pub command_name: String,
    pub command_ns: String,
    pub command_kind: String,
    // TODO: Make started_at an Option<Time> or something similar
    //   when I tried it I got an error that schema-rs doesn't support the Time type
    //   and ran out of time to investigate
    pub started_at: Option<String>,
}

impl TryFrom<DynamicObject> for CommandRef {
    type Error = crate::error::Error;

    fn try_from(command: DynamicObject) -> Result<Self, Self::Error> {
        let report_error = |field: &str| ConversionError {
            message: format!(
                "Error converting to CommandRef from DynamicObject, [{}] cannot be empty!",
                field
            ),
        };

        Ok(CommandRef {
            command_uid: command.metadata.uid.ok_or_else(|| report_error("test"))?,
            command_name: command.metadata.name.ok_or_else(|| report_error("test"))?,
            command_ns: command
                .metadata
                .namespace
                .ok_or_else(|| report_error("test"))?,
            command_kind: command.types.ok_or_else(|| report_error("test"))?.kind,
            started_at: Some(get_current_timestamp()),
        })
    }
}

pub async fn clear_current_command<T>(resource: &mut T, client: &Client) -> OperatorResult<()>
where
    T: CustomResourceExt + Resource + Clone + Debug + DeserializeOwned + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand + Debug + Default + Serialize,
    <T as Resource>::DynamicType: Default,
{
    let resource_clone = resource.clone();
    let status = resource.status_mut().get_or_insert_with(Default::default);

    status.clear_current_command();

    info!("Clearing currentCommand");
    client.merge_patch_status(&resource_clone, &status).await?;
    Ok(())
}

pub async fn maybe_update_current_command<T>(
    resource: &mut T,
    command: &CommandRef,
    client: &Client,
) -> OperatorResult<()>
where
    T: CustomResourceExt + Resource + Clone + Debug + DeserializeOwned + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand + Debug + Default + Serialize,
    <T as Resource>::DynamicType: Default,
{
    let resource_clone = resource.clone();
    let status = resource.status_mut().get_or_insert_with(Default::default);

    if status
        .current_command()
        .filter(|cmd| *cmd != *command)
        .is_none()
    {
        // Current command is none or not equal to the new command -> we need to patch
        // TODO: We need to update the command object in Kubernetes with the start time
        //   not the CommandRef object, but the actual command -
        //   This is the reason why this entire house of cards is not currently doing anything, as
        //   the started_at value for all commands will always be empty and thus the comparison against
        //   the creation time for the pods fails
        status.set_current_command(command.clone());

        info!("Setting currentCommand to [{:?}]", command);

        client.merge_patch_status(&resource_clone, &status).await?;
    }

    Ok(())
}

pub async fn current_command<T>(
    resource: &T,
    resources: &[ApiResource],
    client: &Client,
) -> OperatorResult<Option<CommandRef>>
where
    T: Resource + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand,
{
    warn!("Looking for current command ....");
    match resource.status() {
        None => {
            warn!("No status set..");
            let next_command = get_next_command(resources, client).await;
            warn!("found next command: {:?}", next_command);
            next_command
        }
        Some(status) if status.current_command().is_some() => {
            warn!(
                "Found current command in status: [{:?}]",
                status.current_command()
            );
            Ok(status.current_command())
        }
        Some(_) => {
            warn!("No current command set in status, retrieving from k8s..");
            get_next_command(resources, client).await
        }
    }
}

/// Tries to retrieve the Command for a [`CommandRef`].
pub async fn materialize_command<T>(command_ref: &CommandRef, client: &Client) -> OperatorResult<T>
where
    T: Resource + Clone + Debug + DeserializeOwned,
    <T as Resource>::DynamicType: Default,
{
    client
        .get(&command_ref.command_name, Some(&command_ref.command_ns))
        .await
}

/// Collect and sort all available commands and return the first (the one with
/// the oldest creation timestamp) element.
///
/// # Arguments
/// * `TODO`
/// * `client` - Kubernetes client
///
pub async fn get_next_command(
    resources: &[ApiResource],
    client: &Client,
) -> OperatorResult<Option<CommandRef>> {
    let mut all_commands = collect_commands(resources, client).await?;
    all_commands.sort_by_key(|a| a.metadata.creation_timestamp.clone());
    warn!("all commands: {:?}", all_commands);
    all_commands
        .into_iter()
        .next()
        .map(|bla| bla.try_into())
        .transpose()
}

/// Collect all of a list of resources and returns them in one big list.
///
/// # Arguments
/// * `TODO`
/// * `client` - Kubernetes client
///
async fn collect_commands(
    resources: &[ApiResource],
    client: &Client,
) -> OperatorResult<Vec<DynamicObject>> {
    let mut all_commands = vec![];
    for resource in resources {
        all_commands.append(&mut list_resources(client, resource).await?);
    }
    Ok(all_commands)
}

pub async fn list_resources(
    client: &Client,
    api_resource: &ApiResource,
) -> OperatorResult<Vec<DynamicObject>> {
    let kube_client = client.as_kube_client();
    let api: Api<DynamicObject> = Api::all_with(kube_client, api_resource);
    Ok(api.list(&ListParams::default()).await?.items)
}
