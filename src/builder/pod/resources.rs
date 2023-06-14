use std::{collections::BTreeMap, str::FromStr};

use k8s_openapi::{
    api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity,
};
use tracing::warn;

use crate::{
    commons::resources::ResourceRequirementsType, cpu::CpuQuantity, error::OperatorResult,
    memory::MemoryQuantity,
};

const RESOURCE_DENYLIST: &[&str] = &["cpu", "memory"];

#[derive(Debug, Default)]
pub struct ResourceRequirementsBuilder<CL, CR, ML, MR> {
    other: BTreeMap<String, BTreeMap<ResourceRequirementsType, Quantity>>,
    cpu_request: CR,
    mem_request: MR,
    mem_limit: ML,
    cpu_limit: CL,
}

impl ResourceRequirementsBuilder<(), (), (), ()> {
    pub fn new() -> Self {
        ResourceRequirementsBuilder::default()
    }
}

impl<CR, ML, MR> ResourceRequirementsBuilder<(), CR, ML, MR> {
    pub fn with_cpu_limit(
        self,
        limit: impl Into<String>,
    ) -> ResourceRequirementsBuilder<Quantity, CR, ML, MR> {
        let Self {
            cpu_request,
            mem_request,
            mem_limit,
            other,
            ..
        } = self;

        ResourceRequirementsBuilder {
            cpu_limit: Quantity(limit.into()),
            cpu_request,
            mem_request,
            mem_limit,
            other,
        }
    }

    pub fn with_cpu_range(
        self,
        request: impl Into<String>,
        factor: usize,
    ) -> OperatorResult<ResourceRequirementsBuilder<Quantity, Quantity, ML, MR>> {
        let request = CpuQuantity::from_str(&request.into())?;
        let limit = request * factor;

        let Self {
            mem_request,
            mem_limit,
            other,
            ..
        } = self;

        Ok(ResourceRequirementsBuilder {
            cpu_request: request.into(),
            cpu_limit: limit.into(),
            mem_request,
            mem_limit,
            other,
        })
    }
}

impl<ML, MR> ResourceRequirementsBuilder<Quantity, (), ML, MR> {
    pub fn with_cpu_request(
        self,
        request: impl Into<String>,
    ) -> ResourceRequirementsBuilder<Quantity, Quantity, ML, MR> {
        let Self {
            mem_request,
            cpu_limit,
            mem_limit,
            other,
            ..
        } = self;

        ResourceRequirementsBuilder {
            cpu_request: Quantity(request.into()),
            mem_request,
            cpu_limit,
            mem_limit,
            other,
        }
    }
}

impl<CL, CR, MR> ResourceRequirementsBuilder<CL, CR, (), MR> {
    pub fn with_memory_limit(
        self,
        limit: impl Into<String>,
    ) -> ResourceRequirementsBuilder<CL, CR, Quantity, MR> {
        let Self {
            cpu_request,
            mem_request,
            cpu_limit,
            other,
            ..
        } = self;

        ResourceRequirementsBuilder {
            mem_limit: Quantity(limit.into()),
            cpu_request,
            mem_request,
            cpu_limit,
            other,
        }
    }

    pub fn with_memory_range(
        self,
        request: impl Into<String>,
        factor: f32,
    ) -> OperatorResult<ResourceRequirementsBuilder<CL, CR, Quantity, Quantity>> {
        let request = MemoryQuantity::from_str(&request.into())?;
        let limit = request * factor;

        let Self {
            cpu_request,
            cpu_limit,
            other,
            ..
        } = self;

        Ok(ResourceRequirementsBuilder {
            mem_request: request.into(),
            mem_limit: limit.into(),
            cpu_request,
            cpu_limit,
            other,
        })
    }
}

impl<CL, CR> ResourceRequirementsBuilder<CL, CR, Quantity, ()> {
    pub fn with_memory_request(
        self,
        request: impl Into<String>,
    ) -> ResourceRequirementsBuilder<CL, CR, Quantity, Quantity> {
        let Self {
            cpu_request,
            cpu_limit,
            mem_limit,
            other,
            ..
        } = self;

        ResourceRequirementsBuilder {
            mem_request: Quantity(request.into()),
            cpu_request,
            cpu_limit,
            mem_limit,
            other,
        }
    }
}

impl<CL, CR, ML, MR> ResourceRequirementsBuilder<CL, CR, ML, MR> {
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

        match self.other.get_mut(&resource) {
            Some(types) => {
                if types.contains_key(&rr_type) {
                    warn!(
                        "resource {} for '{}' already set, not overwriting",
                        rr_type, resource
                    );
                }

                types.insert(rr_type, Quantity(quantity.into()));
            }
            None => {
                let types = BTreeMap::from([(rr_type, Quantity(quantity.into()))]);
                self.other.insert(resource, types);
            }
        }

        self
    }
}

impl ResourceRequirementsBuilder<Quantity, Quantity, Quantity, Quantity> {
    pub fn build(self) -> ResourceRequirements {
        let mut limits: BTreeMap<String, Quantity> = BTreeMap::new();
        let mut requests: BTreeMap<String, Quantity> = BTreeMap::new();

        limits.insert("cpu".into(), self.cpu_limit);
        requests.insert("cpu".into(), self.cpu_request);

        limits.insert("memory".into(), self.mem_limit);
        requests.insert("memory".into(), self.mem_request);

        // Insert all other resources not covered by the with_cpu_* and
        // with_memory_* methods.
        for (resource, types) in self.other {
            for (rr_type, quantity) in types {
                match rr_type {
                    ResourceRequirementsType::Limits => limits.insert(resource.clone(), quantity),
                    ResourceRequirementsType::Requests => {
                        requests.insert(resource.clone(), quantity)
                    }
                };
            }
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
                    ("nvidia.com/gpu".into(), Quantity("2".into())),
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
            .with_resource(ResourceRequirementsType::Limits, "nvidia.com/gpu", "2")
            .with_resource(ResourceRequirementsType::Requests, "nvidia.com/gpu", "1")
            .build();

        assert_eq!(rr, resources)
    }
}
