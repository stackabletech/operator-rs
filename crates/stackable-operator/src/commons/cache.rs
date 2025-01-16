use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::time::Duration;

/// Least Recently Used (LRU) cache with per-entry time-to-live (TTL) value.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtlCache {
    /// Time to live per entry; Entries which were not queried within the given duration, are
    /// removed.
    #[serde(default = "TtlCache::default_entry_time_to_live")]
    pub entry_time_to_live: Duration,

    /// Maximum number of entries in the cache; If this threshold is reached then the least
    /// recently used item is removed.
    #[serde(default = "TtlCache::default_max_entries")]
    pub max_entries: i32,
}

impl TtlCache {
    const fn default_entry_time_to_live() -> Duration {
        Duration::from_secs(30)
    }

    const fn default_max_entries() -> i32 {
        1000
    }
}

impl Default for TtlCache {
    fn default() -> Self {
        Self {
            entry_time_to_live: Self::default_entry_time_to_live(),
            max_entries: Self::default_max_entries(),
        }
    }
}
