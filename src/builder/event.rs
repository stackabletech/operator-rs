use chrono::Utc;
use k8s_openapi::{
    api::core::v1::{Event, EventSource, ObjectReference},
    apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time},
};
use kube::{Resource, ResourceExt};

/// Type of Event.
/// The restriction to these two values is not hardcoded in Kubernetes but by convention only.
#[derive(Clone, Debug)]
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

/// A builder to build [`Event`] objects.
///
/// This is mainly useful for tests.
#[derive(Clone, Debug, Default)]
pub struct EventBuilder {
    name: String,
    involved_object: ObjectReference,
    event_type: Option<EventType>,
    action: Option<String>,
    reason: Option<String>,
    message: Option<String>,
    reporting_component: Option<String>,
    reporting_instance: Option<String>,
}

impl EventBuilder {
    /// Creates a new [`EventBuilder`].
    ///
    /// # Arguments
    ///
    /// - `resource` - The resource for which this event is created, will be used to create the `involvedObject` and `metadata.name` fields
    pub fn new<T>(resource: &T) -> EventBuilder
    where
        T: Resource<DynamicType = ()>,
    {
        let involved_object = ObjectReference {
            api_version: Some(T::api_version(&()).to_string()),
            field_path: None,
            kind: Some(T::kind(&()).to_string()),
            name: resource.meta().name.clone(),
            namespace: resource.namespace(),
            resource_version: resource.meta().resource_version.clone(),
            uid: resource.meta().uid.clone(),
        };

        EventBuilder {
            name: resource.name(),
            involved_object,
            ..EventBuilder::default()
        }
    }

    pub fn event_type(&mut self, event_type: &EventType) -> &mut Self {
        self.event_type = Some(event_type.clone());
        self
    }

    /// What action was taken/failed regarding to the Regarding object (e.g. Create, Update, Delete, Reconcile, ...)
    pub fn action(&mut self, action: impl Into<String>) -> &mut Self {
        self.action = Some(action.into());
        self
    }

    /// This should be a short, machine understandable string that gives the reason for this event being generated (e.g. PodMissing, UpdateRunning, ...)
    pub fn reason(&mut self, reason: impl Into<String>) -> &mut Self {
        self.reason = Some(reason.into());
        self
    }

    /// A human-readable description of the status of this operation.
    pub fn message(&mut self, message: impl Into<String>) -> &mut Self {
        self.message = Some(message.into());
        self
    }

    /// Name of the controller that emitted this Event, e.g. `kubernetes.io/kubelet`.
    pub fn reporting_component(&mut self, reporting_component: impl Into<String>) -> &mut Self {
        self.reporting_component = Some(reporting_component.into());
        self
    }

    /// ID of the controller instance, e.g. `kubelet-xyzf`.
    pub fn reporting_instance(&mut self, reporting_instance: impl Into<String>) -> &mut Self {
        self.reporting_instance = Some(reporting_instance.into());
        self
    }

    pub fn build(&self) -> Event {
        let time = Utc::now();

        let source = Some(EventSource {
            component: self.reporting_component.clone(),
            host: None,
        });

        Event {
            action: self.action.clone(),
            count: Some(1),
            event_time: Some(MicroTime(time)),
            first_timestamp: Some(Time(time)),
            involved_object: self.involved_object.clone(),
            last_timestamp: Some(Time(time)),
            message: self.message.clone(),
            metadata: ObjectMeta {
                generate_name: Some(format!("{}-", self.name)),
                ..ObjectMeta::default()
            },
            reason: self.reason.clone(),
            related: None,
            reporting_component: self.reporting_component.clone(),
            reporting_instance: self.reporting_instance.clone(),
            series: None,
            source,
            type_: self
                .event_type
                .as_ref()
                .map(|event_type| event_type.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::pod::PodBuilder;

    #[test]
    fn test_event_builder() {
        let pod = PodBuilder::new()
            .metadata_builder(|builder| builder.name("testpod"))
            .build()
            .unwrap();

        let event = EventBuilder::new(&pod)
            .event_type(&EventType::Normal)
            .action("action")
            .message("message")
            .build();

        assert!(matches!(event.involved_object.kind, Some(pod_name) if pod_name == "Pod"));
        assert!(matches!(event.message, Some(message) if message == "message"));
        assert!(matches!(event.reason, None));
    }
}
