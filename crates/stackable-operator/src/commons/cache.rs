use std::marker::PhantomData;

use educe::Educe;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::time::Duration;

/// [`TtlCache`] with sensible defaults for a user information cache
pub type UserInformationCache = TtlCache<UserInformationCacheDefaults>;

/// Default tunings for [`UserInformationCache`].
#[derive(JsonSchema)]
pub struct UserInformationCacheDefaults;

impl TtlCacheDefaults for UserInformationCacheDefaults {
    fn entry_time_to_live() -> Duration {
        Duration::from_secs(30)
    }

    fn max_entries() -> u32 {
        10_000
    }
}

/// Structure to configure a TTL cache in a product.
#[derive(Deserialize, Educe, JsonSchema, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "D: TtlCacheDefaults", serialize = "D: TtlCacheDefaults")
)]
#[schemars(
    description = "Least Recently Used (LRU) cache with per-entry time-to-live (TTL) value.",
    // We don't care about the fields, but we also use JsonSchema to derive the name for the composite type
    bound(serialize = "D: TtlCacheDefaults + JsonSchema")
)]
#[educe(
    Clone(bound = false),
    Debug(bound = false),
    PartialEq(bound = false),
    Eq
)]
pub struct TtlCache<D> {
    /// Time to live per entry; Entries which were not queried within the given duration, are
    /// removed.
    #[serde(default = "D::entry_time_to_live")]
    pub entry_time_to_live: Duration,

    /// Maximum number of entries in the cache; If this threshold is reached then the least
    /// recently used item is removed.
    #[serde(default = "D::max_entries")]
    pub max_entries: u32,

    #[serde(skip)]
    pub _defaults: PhantomData<D>,
}

impl<D: TtlCacheDefaults> Default for TtlCache<D> {
    fn default() -> Self {
        Self {
            entry_time_to_live: D::entry_time_to_live(),
            max_entries: D::max_entries(),
            _defaults: PhantomData,
        }
    }
}

/// A set of default values for [`TtlCache`].
///
/// This is extracted to a separate trait in order to be able to provide different
/// default tunings for different use cases.
pub trait TtlCacheDefaults {
    /// The default TTL the entries should have.
    fn entry_time_to_live() -> Duration;
    /// The default for the maximum number of entries
    fn max_entries() -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    type MyCache = TtlCache<UserInformationCacheDefaults>;

    #[test]
    fn test_defaults() {
        let my_cache: MyCache = Default::default();
        assert_eq!(my_cache.entry_time_to_live, Duration::from_secs(30));
        assert_eq!(my_cache.max_entries, 10_000);
    }

    #[test]
    fn test_deserialization_defaults() {
        let my_cache: MyCache = serde_json::from_str("{}").unwrap();

        assert_eq!(my_cache.entry_time_to_live, Duration::from_secs(30));
        assert_eq!(my_cache.max_entries, 10_000);
    }

    #[test]
    fn test_deserialization() {
        let my_cache: MyCache = serde_yaml::from_str("entryTimeToLive: 13h\n").unwrap();

        assert_eq!(
            my_cache.entry_time_to_live,
            Duration::from_hours_unchecked(13)
        );
        // As the field is not specified we default
        assert_eq!(my_cache.max_entries, 10_000);
    }
}
