//! Wrapper around serde_yaml which adheres to the YAML specification
//!
//! Operators should use this module instead of serde_yaml.
use serde::ser;

use crate::error::OperatorResult;

pub use serde_yaml::from_str;
pub use serde_yaml::Error;

/// Serialize the given data structure as a String of YAML.
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// return an error.
pub fn to_string<T>(value: &T) -> OperatorResult<String>
where
    T: ?Sized + ser::Serialize,
{
    let yaml = serde_yaml::to_string(value)?;
    Ok(format!("---\n{}", yaml))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn yaml_with_leading_dashes_can_be_deserialized() {
        let yaml = "\
            ---\n\
            key: value";

        let actual_value: BTreeMap<_, _> = from_str(yaml).expect("deserializable value");

        let expected_value: BTreeMap<_, _> = [("key", "value")].into();

        assert_eq!(expected_value, actual_value);
    }

    #[test]
    fn yaml_without_leading_dashes_can_be_deserialized() {
        let yaml = "key: value";

        let actual_value: BTreeMap<_, _> = from_str(yaml).expect("deserializable value");

        let expected_value: BTreeMap<_, _> = [("key", "value")].into();

        assert_eq!(expected_value, actual_value);
    }

    #[test]
    fn value_can_be_serialized() {
        let value: BTreeMap<_, _> = [("key", "value")].into();

        let actual_yaml = to_string(&value).expect("serializable value");

        let expected_yaml = "\
            ---\n\
            key: value\n";

        assert_eq!(expected_yaml, actual_yaml);
    }
}
