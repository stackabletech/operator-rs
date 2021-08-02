use crate::client::Client;
use crate::command_controller;
use crate::error::OperatorResult;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::serde::de::DeserializeOwned;
use kube::api::{ApiResource, DynamicObject, ListParams, Resource};
use kube::Api;
use std::fmt::Debug;

/// Retrieve a timestamp in format: "2021-03-23T16:20:19Z".
/// Required to set command start and finish timestamps.
pub fn get_current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentCommand {
    pub command_uid: String,
    pub command_kind: String,
    pub started_at: String,
}

pub trait HasCurrentCommand {
    fn current_command(&self) -> Option<&CurrentCommand>;

    // TODO: setters are non-rusty, is there a better way? Dirkjan?
    fn set_current_command(&mut self, command: CurrentCommand) -> &CurrentCommand;
}

pub async fn current_command<T>(mut resource: T, resources: &[ApiResource], client: &Client)
where
    T: HasCurrentCommand,
{
    let current_command = match resource.current_command() {
        None => {
            let new_current_command = get_next_command(resources, client).await?.unwrap();
            let new_current_command = CurrentCommand {
                command_uid: new_current_command.metadata.uid.unwrap().clone(),
                command_kind: new_current_command.types.unwrap().kind,
                started_at: get_current_timestamp(),
            };
            resource.set_current_command(new_current_command)
        }
        Some(command) => command.clone(),
    };

    ()
}

pub async fn process_command(resources: &[ApiResource], client: &Client) -> OperatorResult<()> {
    let command = get_next_command(resources, client).await?;
    if let Some(command) = command {
        let kind = command.types.unwrap().kind;

        match kind.as_str() {
            "Pod" => handle_pod(serde_json::from_value(command.data).unwrap()),
            _ => println!("nothing"),
        }
    }

    Ok(())
}

fn handle_pod(pod: Pod) {
    todo!()
}

/// Collect and sort all available commands and return the first (the one with
/// the oldest creation timestamp) element.
///
/// # Arguments
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

/// Collect all different commands in one vector.
///
/// # Arguments
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
