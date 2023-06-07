use std::{collections::BTreeMap, marker::PhantomData};

use k8s_openapi::{
    api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity,
};
use tracing::warn;

use crate::commons::resources::ResourceRequirementsType;

const RESOURCE_DENYLIST: &[&str] = &["cpu", "memory"];

mod state {
    #[derive(Debug, Default)]
    pub struct Initial {}
    pub struct CpuLimitSet {}
    pub struct MemLimitSet {}
    pub struct Final {}
}

#[derive(Debug, Default)]
pub struct ResourceRequirementsBuilder<S = state::Initial> {
    cpu_limit: Option<Quantity>,
    cpu_request: Option<Quantity>,
    mem_limit: Option<Quantity>,
    mem_request: Option<Quantity>,
    other: BTreeMap<String, (ResourceRequirementsType, Quantity)>,
    state: PhantomData<S>,
}

impl ResourceRequirementsBuilder<state::Initial> {
    pub fn new() -> Self {
        ResourceRequirementsBuilder::default()
    }

    pub fn with_cpu_limit(
        self,
        limit: impl Into<String>,
    ) -> ResourceRequirementsBuilder<state::CpuLimitSet> {
        ResourceRequirementsBuilder {
            cpu_limit: Some(Quantity(limit.into())),
            cpu_request: self.cpu_request,
            mem_limit: self.mem_limit,
            mem_request: self.mem_request,
            other: self.other,
            state: PhantomData,
        }
    }

    pub fn with_memory_limit(
        self,
        limit: impl Into<String>,
    ) -> ResourceRequirementsBuilder<state::MemLimitSet> {
        ResourceRequirementsBuilder {
            cpu_limit: self.cpu_limit,
            cpu_request: self.cpu_request,
            mem_limit: Some(Quantity(limit.into())),
            mem_request: self.mem_request,
            other: self.other,
            state: PhantomData,
        }
    }
}

impl ResourceRequirementsBuilder<state::MemLimitSet> {
    pub fn with_cpu_limit(
        self,
        limit: impl Into<String>,
    ) -> ResourceRequirementsBuilder<state::Final> {
        ResourceRequirementsBuilder {
            cpu_limit: Some(Quantity(limit.into())),
            cpu_request: self.cpu_request,
            mem_limit: self.mem_limit,
            mem_request: self.mem_request,
            other: self.other,
            state: PhantomData,
        }
    }
}

impl ResourceRequirementsBuilder<state::CpuLimitSet> {
    pub fn with_memory_limit(
        self,
        limit: impl Into<String>,
    ) -> ResourceRequirementsBuilder<state::Final> {
        ResourceRequirementsBuilder {
            cpu_limit: self.cpu_limit,
            cpu_request: self.cpu_request,
            mem_limit: Some(Quantity(limit.into())),
            mem_request: self.mem_request,
            other: self.other,
            state: PhantomData,
        }
    }
}

impl ResourceRequirementsBuilder<state::Final> {
    pub fn build(self) -> ResourceRequirements {
        let mut limits: BTreeMap<String, Quantity> = BTreeMap::new();
        let mut requests: BTreeMap<String, Quantity> = BTreeMap::new();

        // Insert the CPU and memory limits/requests only when they are set
        if let Some(cpu_limit) = self.cpu_limit {
            limits.insert("cpu".into(), cpu_limit);
        }

        if let Some(mem_limit) = self.mem_limit {
            limits.insert("memory".into(), mem_limit);
        }

        if let Some(cpu_request) = self.cpu_request {
            requests.insert("cpu".into(), cpu_request);
        }

        if let Some(mem_request) = self.mem_request {
            requests.insert("memory".into(), mem_request);
        }

        // Insert all other resources not covered by the with_cpu_* and
        // with_memory_* methods.
        for (resource, (rr_type, quantity)) in self.other {
            match rr_type {
                ResourceRequirementsType::Limits => limits.insert(resource, quantity),
                ResourceRequirementsType::Requests => requests.insert(resource, quantity),
            };
        }

        // Only add limits/requests when there is actually stuff to add
        let limits = if limits.is_empty() {
            None
        } else {
            Some(limits)
        };

        let requests = if requests.is_empty() {
            None
        } else {
            Some(requests)
        };

        ResourceRequirements {
            limits,
            requests,
            ..Default::default()
        }
    }
}

impl<S> ResourceRequirementsBuilder<S> {
    pub fn with_cpu_request(mut self, request: impl Into<String>) -> Self {
        self.cpu_request = Some(Quantity(request.into()));
        self
    }

    pub fn with_memory_request(mut self, request: impl Into<String>) -> Self {
        self.mem_request = Some(Quantity(request.into()));
        self
    }

    pub fn with_resource(
        mut self,
        rr_type: ResourceRequirementsType,
        resource: &str,
        quantity: impl Into<String>,
    ) -> Self {
        if RESOURCE_DENYLIST.contains(&resource) {
            warn!(
                "setting resource '{}' directly is discouraged - use provided methods instead",
                resource
            );
            return self;
        }

        let resource = resource.to_string();

        if self.other.contains_key(&resource) {
            warn!("resource '{}' already set, not overwriting", resource);
            return self;
        }

        self.other
            .insert(resource, (rr_type, Quantity(quantity.into())));

        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_builder() {
        let resources = ResourceRequirements {
            limits: Some(
                [
                    ("cpu".into(), Quantity("1".into())),
                    ("memory".into(), Quantity("128Mi".into())),
                ]
                .into(),
            ),
            requests: Some(
                [
                    ("cpu".into(), Quantity("500m".into())),
                    ("memory".into(), Quantity("64Mi".into())),
                    ("nvidia.com/gpu".into(), Quantity("1".into())),
                ]
                .into(),
            ),
            ..ResourceRequirements::default()
        };

        let rr = ResourceRequirementsBuilder::new()
            .with_cpu_limit("1")
            .with_cpu_request("500m")
            .with_memory_limit("128Mi")
            .with_memory_request("64Mi")
            .with_resource(ResourceRequirementsType::Requests, "nvidia.com/gpu", "1")
            .build();

        assert_eq!(rr, resources)
    }
}
