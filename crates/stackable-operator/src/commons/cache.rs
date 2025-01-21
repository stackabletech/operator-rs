use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::time::Duration;

/// TtlCache with sensible defaults for a user information cache
pub type UserInformationCache = TtlCache<30, 10_000>;

/// Least Recently Used (LRU) cache with per-entry time-to-live (TTL) value.
///
/// This struct has two const generics, so that different use-cases can have different default
/// values:
///
/// * `D_TTL_SEC` is the default TTL (in seconds) the entries should have.
/// * `D_MAX_ENTRIES` is the default for the maximum number of entries
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[schemars(
    description = "Least Recently Used (LRU) cache with per-entry time-to-live (TTL) value."
)]
pub struct TtlCache<const D_TTL_SEC: u64, const D_MAX_ENTRIES: u32> {
    /// Time to live per entry; Entries which were not queried within the given duration, are
    /// removed.
    #[serde(default = "TtlCache::<D_TTL_SEC, D_MAX_ENTRIES>::default_entry_time_to_live")]
    pub entry_time_to_live: Duration,

    /// Maximum number of entries in the cache; If this threshold is reached then the least
    /// recently used item is removed.
    #[serde(default = "TtlCache::<D_TTL_SEC, D_MAX_ENTRIES>::default_max_entries")]
    pub max_entries: u32,
}

impl<const D_TTL_SEC: u64, const D_MAX_ENTRIES: u32> TtlCache<D_TTL_SEC, D_MAX_ENTRIES> {
    const fn default_entry_time_to_live() -> Duration {
        Duration::from_secs(D_TTL_SEC)
    }

    const fn default_max_entries() -> u32 {
        D_MAX_ENTRIES
    }
}

impl<const D_TTL_SEC: u64, const D_MAX_ENTRIES: u32> Default
    for TtlCache<D_TTL_SEC, D_MAX_ENTRIES>
{
    fn default() -> Self {
        Self {
            entry_time_to_live: Self::default_entry_time_to_live(),
            max_entries: Self::default_max_entries(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type MyCache = TtlCache<30, 10_000>;

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
