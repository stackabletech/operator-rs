use crate::client::Client;
use crate::command_controller;
use crate::error::OperatorResult;
use json_patch::{PatchOperation, ReplaceOperation};
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::api::{ApiResource, DynamicObject, ListParams, Resource};
use kube::Api;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
    fn current_command(&self) -> Option<&CommandRef>;

    // TODO: setters are non-rusty, is there a better way? Dirkjan?
    fn set_current_command(&mut self, command: CommandRef) -> CommandRef;
}

pub trait State {}

pub async fn maybe_update_current_command<T>(
    mut resource: T,
    command: &CommandRef,
    client: &Client,
) -> OperatorResult<()>
where
    T: Resource + HasCurrentCommand + Clone + Debug + DeserializeOwned,
    <T as Resource>::DynamicType: Default,
{
    if resource
        .current_command()
        .filter(|cmd| *cmd != command)
        .is_some()
    {
        // Current command is none or not equal to the new command -> we need to patch

        info!("Setting currentCommand to [{:?}]", command);
        client.merge_patch_status(&resource, &serde_json::json!({ "currentCommand": command }));
    }

    Ok(())
}

pub async fn current_command<T>(
    mut resource: T,
    resources: &[ApiResource],
    client: &Client,
) -> OperatorResult<Option<CommandRef>>
where
    T: HasCurrentCommand,
{
    Ok(match resource.current_command() {
        None => {
            if let Some(new_current_command) = get_next_command(resources, client).await? {
                let new_current_command = CommandRef {
                    command_uid: new_current_command.metadata.uid.unwrap().clone(),
                    command_name: new_current_command.metadata.name.unwrap().clone(),
                    command_ns: new_current_command.metadata.namespace.unwrap().clone(),
                    command_kind: new_current_command.types.unwrap().kind,
                    started_at: get_current_timestamp(),
                };
                Some(resource.set_current_command(new_current_command))
            } else {
                None
            }
        }
        Some(command) => Some(command.clone()),
    }
    .clone())
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
) -> OperatorResult<Option<DynamicObject>> {
    let mut all_commands = collect_commands(resources, client).await?;
    all_commands.sort_by_key(|a| a.metadata.creation_timestamp.clone());
    Ok(all_commands.into_iter().next())
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
