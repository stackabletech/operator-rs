use crate::client::Client;
use crate::error::OperatorResult;
use chrono::Utc;
use k8s_openapi::api::core::v1::{Event, EventSource, ObjectReference};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time};
use k8s_openapi::Resource;
use kube::api::Meta;

pub enum EventType {
    Normal,
    Warning,
}

impl ToString for EventType {
    fn to_string(&self) -> String {
        match self {
            EventType::Normal => "Normal".to_string(),
            EventType::Warning => "Warning".to_string(),
        }
    }
}

/*

         s
const (
    // Event action describes what action was taken
    eventActionReconcile = "Reconcile"
    eventActionCreate    = "Create"
    eventActionUpdate    = "Update"
    eventActionDelete    = "Delete"
)

const (
    // Short, machine understandable string that gives the reason for the transition into the object's current status
    eventReasonReconcileStarted    = "ReconcileStarted"
    eventReasonReconcileInProgress = "ReconcileInProgress"
    eventReasonReconcileCompleted  = "ReconcileCompleted"
    eventReasonReconcileFailed     = "ReconcileFailed"
    eventReasonCreateStarted       = "CreateStarted"
    eventReasonCreateInProgress    = "CreateInProgress"
    eventReasonCreateCompleted     = "CreateCompleted"
    eventReasonCreateFailed        = "CreateFailed"
    eventReasonUpdateStarted       = "UpdateStarted"
    eventReasonUpdateInProgress    = "UpdateInProgress"
    eventReasonUpdateCompleted     = "UpdateCompleted"
    eventReasonUpdateFailed        = "UpdateFailed"
    eventReasonDeleteStarted       = "DeleteStarted"
    eventReasonDeleteInProgress    = "DeleteInProgress"
    eventReasonDeleteCompleted     = "DeleteCompleted"
    eventReasonDeleteFailed        = "DeleteFailed"
)

 */

// action = reconcile, create, update, delete
pub fn create_event<T>(
    resource: &T,
    source: &str,
    event_type: &EventType,
    action: &str,
    reason: &str,
    message: &str,
) -> Event
where
    T: Meta + Resource,
{
    let involved_object = ObjectReference {
        api_version: Some(T::API_VERSION.to_string()),
        field_path: None,
        kind: Some(T::KIND.to_string()),
        name: Some(Meta::name(resource)),
        namespace: Meta::namespace(resource),
        resource_version: resource.meta().resource_version.clone(),
        uid: resource.meta().uid.clone(),
    };

    let source = Some(EventSource {
        component: Some(source.to_string()),
        host: None,
    });

    let time = Utc::now();

    let event = Event {
        action: Some(action.to_string()),
        count: Some(1),
        event_time: Some(MicroTime(time.clone())),
        first_timestamp: Some(Time(time.clone())),
        involved_object,
        last_timestamp: Some(Time(time.clone())),
        message: Some(message.to_string()),
        metadata: ObjectMeta {
            generate_name: Some(format!("{}-", Meta::name(resource))),
            ..ObjectMeta::default()
        },
        reason: Some(reason.to_string()),
        related: None,
        reporting_component: None, // TODO: This should probably be a part of Controller
        reporting_instance: None,
        series: None,
        source,
        type_: Some(event_type.to_string()),
    };

    event
}

pub async fn emit_event(client: &Client, event: &Event) {
    let result = client.create(event).await;
    if let Err(err) = result {
        // TODO: Log error
    }
}
