use crate::types::{Annotation, AnnotationParseError};

pub struct AnnotationListBuilder {
    prefix: Option<String>,
    annotations: Vec<Annotation>,
}

impl AnnotationListBuilder {
    pub fn new<T>(prefix: Option<T>) -> Self
    where
        T: Into<String>,
    {
        Self {
            prefix: prefix.map(Into::into),
            annotations: Vec::new(),
        }
    }

    pub fn add<T>(&mut self, name: T, value: T) -> Result<&mut Self, AnnotationParseError>
    where
        T: Into<String>,
    {
        self.annotations.push(Annotation::new(
            self.prefix.clone(),
            name.into(),
            value.into(),
        )?);

        Ok(self)
    }

    pub fn build(self) -> Vec<Annotation> {
        self.annotations
    }
}
