use chrono::Utc;
use k8s_openapi::api::core::v1::{Event, EventSource, ObjectReference};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time};
use kube::{Resource, ResourceExt};

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

/// Creates an [`Event`] that can be sent to Kubernetes.
///
/// # Arguments
///
/// - `resource` - The resource for which this event is created, will be used to create the `involvedObject` and `metadata.name` fields
/// - `event_type` - Type of this event (Normal, Warning). The restriction to those two values is not hardcoded in Kubernetes but by convention only.
/// - `action` - What action was taken/failed regarding to the Regarding object (e.g. Create, Update, Delete, Reconcile, ...)
/// - `reason` - This should be a short, machine understandable string that gives the reason for this event being generated (e.g. PodMissing, UpdateRunning, ...)
/// - `message` - A human-readable description of the status of this operation.
/// - `reporting_component` - Name of the controller that emitted this Event, e.g. `kubernetes.io/kubelet`.
/// - `reporting_instance` - ID of the controller instance, e.g. `kubelet-xyzf`.
pub fn create_event<T>(
    resource: &T,
    event_type: Option<&EventType>,
    action: Option<&str>,
    reason: Option<&str>,
    message: Option<&str>,
    reporting_component: Option<&str>,
    reporting_instance: Option<&str>,
) -> Event
where
    T: Resource<DynamicType = ()>,
{
    let component = reporting_component.map(String::from);
    let involved_object = ObjectReference {
        api_version: Some(T::api_version(&()).to_string()),
        field_path: None,
        kind: Some(T::kind(&()).to_string()),
        name: resource.meta().name.clone(),
        namespace: resource.namespace(),
        resource_version: resource.meta().resource_version.clone(),
        uid: resource.meta().uid.clone(),
    };

    let source = Some(EventSource {
        component: component.clone(),
        host: None,
    });

    let time = Utc::now();

    let event = Event {
        action: action.map(String::from),
        count: Some(1),
        event_time: Some(MicroTime(time)),
        first_timestamp: Some(Time(time)),
        involved_object,
        last_timestamp: Some(Time(time)),
        message: message.map(String::from),
        metadata: ObjectMeta {
            generate_name: Some(format!("{}-", resource.name())),
            ..ObjectMeta::default()
        },
        reason: reason.map(String::from),
        related: None,
        reporting_component: component,
        reporting_instance: reporting_instance.map(String::from),
        series: None,
        source,
        type_: event_type.map(|event_type| event_type.to_string()),
    };

    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::PodBuilder;

    #[test]
    fn test_create_event() {
        let pod = PodBuilder::new().name("testpod").build();
        let event = create_event(
            &pod,
            Some(&EventType::Normal),
            Some("action"),
            None,
            Some("message"),
            None,
            None,
        );

        assert!(
            matches!(event.involved_object.kind, Some(pod_name) if pod_name == "Pod".to_string())
        );

        assert!(matches!(event.message, Some(message) if message == "message".to_string()));
        assert!(matches!(event.reason, None));
    }
}
