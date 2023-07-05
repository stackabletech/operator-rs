use std::collections::BTreeMap;

use crate::types::{Annotation, KeyValuePairExt, KeyValuePairParseError, Label};

pub type AnnotationListBuilder = KeyValuePairBuilder<Annotation>;
pub type LabelListBuilder = KeyValuePairBuilder<Label>;

pub struct KeyValuePairBuilder<P: KeyValuePairExt> {
    prefix: Option<String>,
    kvps: Vec<P>,
}

impl<P: KeyValuePairExt> KeyValuePairBuilder<P> {
    pub fn new<T>(prefix: Option<T>) -> Self
    where
        T: Into<String>,
    {
        Self {
            prefix: prefix.map(Into::into),
            kvps: Vec::new(),
        }
    }

    pub fn add<T>(&mut self, name: T, value: T) -> Result<&mut Self, KeyValuePairParseError>
    where
        T: Into<String>,
    {
        let kvp = P::new(self.prefix.clone(), name.into(), value.into())?;
        self.kvps.push(kvp);
        Ok(self)
    }

    pub fn build(self) -> BTreeMap<String, P> {
        self.kvps.iter().map(|a| (a.key(), a.clone())).collect()
    }
}
