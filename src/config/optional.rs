use crate::config::merge::Merge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use stackable_operator_derive::Optional;

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Complex<T>(Option<T>);

impl<T> Complex<T> {
    pub fn get(self) -> Option<T> {
        self.0
    }
}

impl<T: Clone + Merge> Merge for Complex<T> {
    fn merge(&mut self, defaults: &Self) {
        match (&mut self.0, &defaults.0) {
            (Some(s), Some(d)) => s.merge(d),
            (None, Some(d)) => self.0 = Some(d).cloned(),
            (_, _) => {}
        }
    }
}

#[cfg(test)]
mod tests {}
