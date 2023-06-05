use std::{collections::BTreeMap, marker::PhantomData};

use k8s_openapi::{
    api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity,
};

mod state {
    pub struct Initial {}
    pub struct MissingCpuLimit {}
    pub struct MissingMemLimit {}
    pub struct Final {}
}

#[derive(Debug)]
pub struct ResourceRequirementsBuilder<S = state::Initial> {
    cpu_limit: Option<Quantity>,
    cpu_request: Option<Quantity>,
    mem_limit: Option<Quantity>,
    mem_request: Option<Quantity>,
    state: PhantomData<S>,
}

impl Default for ResourceRequirementsBuilder {
    fn default() -> Self {
        Self {
            cpu_limit: Default::default(),
            cpu_request: Default::default(),
            mem_limit: Default::default(),
            mem_request: Default::default(),
            state: Default::default(),
        }
    }
}

impl ResourceRequirementsBuilder<state::Initial> {
    pub fn new() -> Self {
        ResourceRequirementsBuilder::default()
    }

    pub fn with_cpu_limit(
        self,
        limit: Quantity,
    ) -> ResourceRequirementsBuilder<state::MissingMemLimit> {
        ResourceRequirementsBuilder {
            cpu_limit: Some(limit),
            cpu_request: self.cpu_request,
            mem_limit: self.mem_limit,
            mem_request: self.mem_request,
            state: PhantomData,
        }
    }

    pub fn with_memory_limit(
        self,
        limit: Quantity,
    ) -> ResourceRequirementsBuilder<state::MissingCpuLimit> {
        ResourceRequirementsBuilder {
            cpu_limit: self.cpu_limit,
            cpu_request: self.cpu_request,
            mem_limit: Some(limit),
            mem_request: self.mem_request,
            state: PhantomData,
        }
    }
}

impl ResourceRequirementsBuilder<state::MissingCpuLimit> {
    pub fn with_cpu_limit(self, limit: Quantity) -> ResourceRequirementsBuilder<state::Final> {
        ResourceRequirementsBuilder {
            cpu_limit: Some(limit),
            cpu_request: self.cpu_request,
            mem_limit: self.mem_limit,
            mem_request: self.mem_request,
            state: PhantomData,
        }
    }
}

impl ResourceRequirementsBuilder<state::MissingMemLimit> {
    pub fn with_memory_limit(self, limit: Quantity) -> ResourceRequirementsBuilder<state::Final> {
        ResourceRequirementsBuilder {
            cpu_limit: self.cpu_limit,
            cpu_request: self.cpu_request,
            mem_limit: Some(limit),
            mem_request: self.mem_request,
            state: PhantomData,
        }
    }
}

impl ResourceRequirementsBuilder<state::Final> {
    pub fn build(self) -> ResourceRequirements {
        let mut limits: BTreeMap<String, Quantity> = BTreeMap::new();
        let mut requests: BTreeMap<String, Quantity> = BTreeMap::new();

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

        ResourceRequirements {
            limits: Some(limits),
            requests: Some(requests),
            ..Default::default()
        }
    }
}

impl<S> ResourceRequirementsBuilder<S> {
    pub fn with_cpu_request(mut self, request: Quantity) -> Self {
        self.cpu_request = Some(request);
        self
    }

    pub fn with_memory_request(mut self, request: Quantity) -> Self {
        self.mem_request = Some(request);
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_builder() {
        let rr = ResourceRequirementsBuilder::new()
            .with_cpu_limit(Quantity("1".into()))
            .with_cpu_request(Quantity("500m".into()))
            .with_memory_limit(Quantity("128Mi".into()))
            .with_memory_request(Quantity("64Mi".into()))
            .build();
    }
}
