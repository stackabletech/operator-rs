use crate::error::{Error, OperatorResult};
use k8s_openapi::{api::core::v1::ConfigMap, apimachinery::pkg::apis::meta::v1::ObjectMeta};
use std::collections::BTreeMap;

/// A builder to build [`ConfigMap`] objects.
#[derive(Clone, Default)]
pub struct ConfigMapBuilder {
    metadata: Option<ObjectMeta>,
    data: Option<BTreeMap<String, String>>,
}

impl ConfigMapBuilder {
    pub fn new() -> ConfigMapBuilder {
        ConfigMapBuilder::default()
    }

    pub fn metadata_default(&mut self) -> &mut Self {
        self.metadata(ObjectMeta::default());
        self
    }

    pub fn metadata(&mut self, metadata: impl Into<ObjectMeta>) -> &mut Self {
        self.metadata = Some(metadata.into());
        self
    }

    pub fn metadata_opt(&mut self, metadata: impl Into<Option<ObjectMeta>>) -> &mut Self {
        self.metadata = metadata.into();
        self
    }

    pub fn add_data(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.data
            .get_or_insert_with(BTreeMap::new)
            .insert(key.into(), value.into());
        self
    }

    pub fn data(&mut self, data: BTreeMap<String, String>) -> &mut Self {
        self.data = Some(data);
        self
    }

    pub fn build(&self) -> OperatorResult<ConfigMap> {
        Ok(ConfigMap {
            metadata: match self.metadata {
                None => return Err(Error::MissingObjectKey { key: "metadata" }),
                Some(ref metadata) => metadata.clone(),
            },
            data: self.data.clone(),
            ..ConfigMap::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::{configmap::ConfigMapBuilder, resource::ObjectMetaBuilder};

    use std::collections::BTreeMap;

    #[test]
    fn test_configmap_builder() {
        let mut data = BTreeMap::new();
        data.insert("foo".to_string(), "bar".to_string());
        let configmap = ConfigMapBuilder::new()
            .data(data)
            .add_data("bar", "foo")
            .metadata_opt(Some(ObjectMetaBuilder::new().name("test").build()))
            .build()
            .unwrap();

        assert!(matches!(configmap.data.as_ref().unwrap().get("foo"), Some(bar) if bar == "bar"));
        assert!(matches!(configmap.data.as_ref().unwrap().get("bar"), Some(bar) if bar == "foo"));
    }
}
