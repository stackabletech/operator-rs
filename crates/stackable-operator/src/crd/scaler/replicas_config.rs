//! Replica configuration for Stackable role groups.
//!
//! [`ReplicasConfig`] replaces the simple `replicas: Option<u16>` field on role groups,
//! allowing operators to express fixed counts, HPA-managed scaling, Stackable auto-scaling,
//! or externally-managed replicas in a single enum.

use std::borrow::Cow;

use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscalerSpec;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::Snafu;

/// Errors returned by [`ReplicasConfig::validate`].
#[derive(Debug, Snafu)]
pub enum ValidationError {
    /// A `Fixed(0)` replica count is not allowed.
    #[snafu(display("fixed replica count must be at least 1, got 0"))]
    FixedZero,

    /// The `min_replicas` field in [`AutoConfig`] must be at least 1.
    #[snafu(display("auto min_replicas must be at least 1, got {min}"))]
    AutoMinZero {
        /// The invalid minimum replica count.
        min: u16,
    },

    /// The `max_replicas` must be greater than or equal to `min_replicas`.
    #[snafu(display("auto max_replicas ({max}) must be >= min_replicas ({min})"))]
    AutoMaxLessThanMin {
        /// The minimum replica count.
        min: u16,
        /// The maximum replica count that was less than `min`.
        max: u16,
    },
}

/// Configuration for a Kubernetes `HorizontalPodAutoscaler` that manages the role group.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HpaConfig {
    /// The HPA spec to apply. The `scaleTargetRef` and `minReplicas` fields are managed
    /// by the operator and will be overwritten.
    pub spec: HorizontalPodAutoscalerSpec,
}

/// Configuration for Stackable-managed auto-scaling.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AutoConfig {
    /// Minimum number of replicas the auto-scaler may scale down to.
    pub min_replicas: u16,
    /// Maximum number of replicas the auto-scaler may scale up to.
    pub max_replicas: u16,
}

/// How replicas are managed for a role group.
///
/// This enum supports multiple input formats for ergonomic YAML/JSON authoring:
///
/// - A bare integer (e.g. `3`) is parsed as `Fixed(3)`.
/// - The string `"externallyScaled"` is parsed as `ExternallyScaled`.
/// - An object with a discriminant key (`fixed`, `hpa`, or `auto`) selects the
///   corresponding variant.
///
/// # Validation
///
/// After deserialization, call [`validate`](ReplicasConfig::validate) to enforce
/// business rules (e.g. `Fixed(0)` is not allowed).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplicasConfig {
    /// A fixed number of replicas managed by the operator.
    Fixed(u16),
    /// Replicas managed by a Kubernetes `HorizontalPodAutoscaler`.
    Hpa(Box<HpaConfig>),
    /// Replicas managed by the Stackable auto-scaler.
    Auto(AutoConfig),
    /// Replicas managed by an external system. The operator creates a
    /// [`StackableScaler`](super::v1alpha1::StackableScaler) that exposes
    /// a `/scale` subresource for external controllers to target.
    ExternallyScaled,
}

impl Default for ReplicasConfig {
    fn default() -> Self {
        Self::Fixed(1)
    }
}

impl ReplicasConfig {
    /// Validates business rules for this configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::FixedZero`] if the variant is `Fixed(0)`.
    /// Returns [`ValidationError::AutoMinZero`] if `min_replicas` is 0.
    /// Returns [`ValidationError::AutoMaxLessThanMin`] if `max_replicas < min_replicas`.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Fixed(0) => FixedZeroSnafu.fail(),
            Self::Auto(cfg) if cfg.min_replicas == 0 => AutoMinZeroSnafu {
                min: cfg.min_replicas,
            }
            .fail(),
            Self::Auto(cfg) if cfg.max_replicas < cfg.min_replicas => AutoMaxLessThanMinSnafu {
                min: cfg.min_replicas,
                max: cfg.max_replicas,
            }
            .fail(),
            _ => Ok(()),
        }
    }
}

impl JsonSchema for ReplicasConfig {
    fn schema_name() -> Cow<'static, str> {
        "ReplicasConfig".into()
    }

    fn json_schema(generator: &mut schemars::generate::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Replica configuration for a role group.",
            "oneOf": [
                {
                    "description": "Fixed replica count (bare integer).",
                    "type": "integer",
                    "minimum": 1
                },
                {
                    "description": "Externally managed replicas.",
                    "type": "string",
                    "const": "externallyScaled"
                },
                {
                    "description": "Fixed replica count (object form).",
                    "type": "object",
                    "required": ["fixed"],
                    "properties": {
                        "fixed": { "type": "integer", "minimum": 1 }
                    },
                    "additionalProperties": false
                },
                {
                    "description": "HPA-managed replicas.",
                    "type": "object",
                    "required": ["hpa"],
                    "properties": {
                        "hpa": generator.subschema_for::<HpaConfig>()
                    },
                    "additionalProperties": false
                },
                {
                    "description": "Stackable auto-scaling.",
                    "type": "object",
                    "required": ["auto"],
                    "properties": {
                        "auto": generator.subschema_for::<AutoConfig>()
                    },
                    "additionalProperties": false
                }
            ]
        })
    }
}

impl<'de> Deserialize<'de> for ReplicasConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct ReplicasConfigVisitor;

        impl<'de> Visitor<'de> for ReplicasConfigVisitor {
            type Value = ReplicasConfig;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(
                    "an integer, the string \"externallyScaled\", \
                     or an object with key \"fixed\", \"hpa\", or \"auto\"",
                )
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                let value = u16::try_from(value)
                    .map_err(|_| de::Error::custom("integer out of u16 range"))?;
                Ok(ReplicasConfig::Fixed(value))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                let value = u16::try_from(value)
                    .map_err(|_| de::Error::custom("integer out of u16 range"))?;
                Ok(ReplicasConfig::Fixed(value))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match value {
                    "externallyScaled" => Ok(ReplicasConfig::ExternallyScaled),
                    other => Err(de::Error::unknown_variant(other, &["externallyScaled"])),
                }
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let key: String = map
                    .next_key()?
                    .ok_or_else(|| de::Error::custom("expected a non-empty object"))?;

                let result = match key.as_str() {
                    "fixed" => {
                        let value: u16 = map.next_value()?;
                        ReplicasConfig::Fixed(value)
                    }
                    "hpa" => {
                        let value: HpaConfig = map.next_value()?;
                        ReplicasConfig::Hpa(Box::new(value))
                    }
                    "auto" => {
                        let value: AutoConfig = map.next_value()?;
                        ReplicasConfig::Auto(value)
                    }
                    other => {
                        return Err(de::Error::unknown_field(other, &["fixed", "hpa", "auto"]));
                    }
                };

                // Drain remaining keys to ensure no extra fields.
                if map.next_key::<String>()?.is_some() {
                    return Err(de::Error::custom(
                        "expected exactly one key (\"fixed\", \"hpa\", or \"auto\")",
                    ));
                }

                Ok(result)
            }
        }

        deserializer.deserialize_any(ReplicasConfigVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_fixed_from_integer() {
        let config: ReplicasConfig = serde_json::from_str("3").expect("should parse integer");
        assert_eq!(config, ReplicasConfig::Fixed(3));
    }

    #[test]
    fn deserialize_fixed_from_object() {
        let config: ReplicasConfig =
            serde_json::from_str(r#"{"fixed": 5}"#).expect("should parse fixed object");
        assert_eq!(config, ReplicasConfig::Fixed(5));
    }

    #[test]
    fn deserialize_externally_scaled() {
        let config: ReplicasConfig =
            serde_json::from_str(r#""externallyScaled""#).expect("should parse string variant");
        assert_eq!(config, ReplicasConfig::ExternallyScaled);
    }

    #[test]
    fn deserialize_hpa() {
        let config: ReplicasConfig =
            serde_json::from_str(r#"{"hpa": {"spec": {"maxReplicas": 10}}}"#)
                .expect("should parse hpa object");
        assert!(matches!(config, ReplicasConfig::Hpa(..)));
    }

    #[test]
    fn deserialize_auto() {
        let config: ReplicasConfig =
            serde_json::from_str(r#"{"auto": {"minReplicas": 2, "maxReplicas": 10}}"#)
                .expect("should parse auto object");
        assert_eq!(
            config,
            ReplicasConfig::Auto(AutoConfig {
                min_replicas: 2,
                max_replicas: 10,
            })
        );
    }

    #[test]
    fn fixed_zero_is_invalid() {
        let config = ReplicasConfig::Fixed(0);
        let result = config.validate();
        assert!(matches!(result, Err(ValidationError::FixedZero)));
    }

    #[test]
    fn auto_min_zero_is_invalid() {
        let config = ReplicasConfig::Auto(AutoConfig {
            min_replicas: 0,
            max_replicas: 5,
        });
        let result = config.validate();
        assert!(matches!(result, Err(ValidationError::AutoMinZero { .. })));
    }

    #[test]
    fn auto_max_less_than_min_is_invalid() {
        let config = ReplicasConfig::Auto(AutoConfig {
            min_replicas: 5,
            max_replicas: 2,
        });
        let result = config.validate();
        assert!(matches!(
            result,
            Err(ValidationError::AutoMaxLessThanMin { .. })
        ));
    }

    #[test]
    fn option_none_defaults_to_fixed_1() {
        let config: Option<ReplicasConfig> =
            serde_json::from_str("null").expect("should parse null");
        assert_eq!(config.unwrap_or_default(), ReplicasConfig::Fixed(1));
    }
}
