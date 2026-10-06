use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// No credential: the product does not authenticate the agent.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Anonymous {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_empty_object() {
        let anonymous: Anonymous =
            serde_json::from_str("{}").expect("an empty object should be valid");

        assert_eq!(Anonymous {}, anonymous);
    }

    #[test]
    fn schema_is_an_object() {
        let schema = serde_json::to_value(schemars::schema_for!(Anonymous))
            .expect("the schema should be serializable");

        assert_eq!("object", schema["type"]);
    }
}
