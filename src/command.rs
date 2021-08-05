use crate::client::Client;
use crate::error::Error::ConversionError;
use crate::error::OperatorResult;
use crate::{command_controller, CustomResourceExt};
use json_patch::{PatchOperation, ReplaceOperation};
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::api::{ApiResource, DynamicObject, ListParams, Resource};
use kube::core::object::HasStatus;
use kube::Api;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use tracing::info;

/// Retrieve a timestamp in format: "2021-03-23T16:20:19Z".
/// Required to set command start and finish timestamps.
pub fn get_current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRef {
    pub command_uid: String,
    pub command_name: String,
    pub command_ns: String,
    pub command_kind: String,
    pub started_at: String,
}

pub trait HasCurrentCommand {
    fn current_command(&self) -> Option<CommandRef>;

    // TODO: setters are non-rusty, is there a better way? Dirkjan?
    fn set_current_command(&mut self, command: CommandRef);
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
            command_uid: command.metadata.uid.ok_or(report_error("test"))?.clone(),
            command_name: command.metadata.name.ok_or(report_error("test"))?.clone(),
            command_ns: command
                .metadata
                .namespace
                .ok_or(report_error("test"))?
                .clone(),
            command_kind: command.types.ok_or(report_error("test"))?.kind,
            started_at: get_current_timestamp(),
        })
    }
}

pub trait State {}

pub async fn maybe_update_current_command<T>(
    resource: &mut T,
    command: &CommandRef,
    client: &Client,
) -> OperatorResult<()>
where
    T: CustomResourceExt
        + Resource
        + Clone
        + Debug
        + DeserializeOwned
        + k8s_openapi::Metadata
        + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand + Debug + Serialize,
    <T as Resource>::DynamicType: Default,
{
    let resource_clone = resource.clone();
    let mut status = resource
        .status_mut()
        .get_or_insert_with(|| Default::default());

    if status
        .current_command()
        .filter(|cmd| *cmd != *command)
        .is_some()
    {
        // Current command is none or not equal to the new command -> we need to patch

        status.set_current_command(command.clone());

        info!("Setting currentCommand to [{:?}]", command);
        client.merge_patch_status(&resource_clone, &status);
    }

    Ok(())
}

pub async fn current_command<T>(
    mut resource: T,
    resources: &[ApiResource],
    client: &Client,
) -> OperatorResult<Option<CommandRef>>
where
    T: Resource + HasStatus,
    <T as HasStatus>::Status: HasCurrentCommand,
{
    match resource.status() {
        None => get_next_command(resources, client).await,
        Some(status) if status.current_command().is_some() => Ok(status.current_command()),
        Some(status) => get_next_command(resources, client).await,
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
