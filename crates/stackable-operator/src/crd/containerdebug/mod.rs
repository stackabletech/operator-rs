//! Configuration of the `containerdebug` tool, which runs in the background of product containers
//! and periodically logs information about the container environment.
//!
//! Learn more in the
//! [container environment documentation](DOCS_BASE_URL_PLACEHOLDER/concepts/observability/containerdebug).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snafu::{Snafu, ensure};
use stackable_shared::time::Duration;

use crate::{
    config::{fragment::Fragment, merge::Merge},
    versioned::versioned,
};

/// The interval used if the user does not configure one.
pub const DEFAULT_INTERVAL: Duration = Duration::from_minutes_unchecked(30);

#[derive(Debug, Eq, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("the containerdebug interval must not be zero"))]
    IntervalZero,
}

#[versioned(version(name = "v1alpha1"))]
pub mod versioned {
    /// Configures the `containerdebug` tool, which periodically logs information about the container
    /// environment (such as network interfaces, DNS lookups and disk usage) in the background of the
    /// product container.
    #[derive(Clone, Debug, Eq, Fragment, JsonSchema, PartialEq)]
    #[fragment(path_overrides(fragment = "crate::config::fragment"))]
    #[fragment_attrs(
        derive(
            Clone,
            Debug,
            Default,
            Deserialize,
            Eq,
            JsonSchema,
            Merge,
            PartialEq,
            Serialize
        ),
        merge(path_overrides(merge = "crate::config::merge")),
        serde(rename_all = "camelCase")
    )]
    pub struct ContainerdebugConfig {
        /// Whether `containerdebug` runs in the background of the product container. Defaults to `true`.
        pub enabled: bool,

        /// Time between two runs of `containerdebug`, e.g. `30m`, `1h` or `2d`. Must not be zero.
        /// Defaults to `30m`.
        pub interval: Duration,
    }
}

impl v1alpha1::ContainerdebugConfig {
    /// Validates the merged configuration.
    ///
    /// The interval is only validated if `containerdebug` is enabled, so that a role group can
    /// disable it without failing on an interval inherited from its role.
    pub fn validate(&self) -> Result<ValidatedContainerdebugConfig, Error> {
        if !self.enabled {
            return Ok(ValidatedContainerdebugConfig::Disabled);
        }

        // `containerdebug` schedules the next run by adding the interval to the start of the
        // previous one, so an interval of zero would collect (including DNS lookups) in a tight loop.
        ensure!(!self.interval.is_zero(), IntervalZeroSnafu);

        Ok(ValidatedContainerdebugConfig::Enabled {
            interval: self.interval,
        })
    }
}

/// A validated [`v1alpha1::ContainerdebugConfig`].
///
/// Unlike the user-facing configuration, this cannot carry an interval while `containerdebug` is
/// disabled, nor an interval of zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidatedContainerdebugConfig {
    Disabled,
    Enabled { interval: Duration },
}

impl ValidatedContainerdebugConfig {
    /// Returns the shell command which starts `containerdebug` in the background, or [`None`] if
    /// it is disabled.
    ///
    /// The collected state is written as JSON to `containerdebug-state.json` in `log_dir`.
    pub fn background_command(&self, log_dir: &str) -> Option<String> {
        match self {
            Self::Disabled => None,
            // `containerdebug` requires the `=` between `--loop` and its value.
            Self::Enabled { interval } => Some(format!(
                "containerdebug --output={log_dir}/containerdebug-state.json --loop={interval} &"
            )),
        }
    }
}

/// Returns the default `containerdebug` configuration, which operators merge underneath the
/// role and role group configuration.
pub fn default_containerdebug_config() -> v1alpha1::ContainerdebugConfigFragment {
    v1alpha1::ContainerdebugConfigFragment {
        enabled: Some(true),
        interval: Some(DEFAULT_INTERVAL),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rstest::rstest;

    use super::*;
    use crate::config::fragment;

    const LOG_DIR: &str = "/stackable/log";

    fn config(enabled: bool, interval: &str) -> v1alpha1::ContainerdebugConfig {
        v1alpha1::ContainerdebugConfig {
            enabled,
            interval: Duration::from_str(interval).expect("valid duration"),
        }
    }

    #[test]
    fn defaults_render_command() {
        let config: v1alpha1::ContainerdebugConfig =
            fragment::validate(default_containerdebug_config()).expect("defaults are complete");

        assert_eq!(
            config
                .validate()
                .expect("defaults are valid")
                .background_command(LOG_DIR),
            Some(
                "containerdebug --output=/stackable/log/containerdebug-state.json --loop=30m &"
                    .to_owned()
            )
        );
    }

    #[rstest]
    #[case("0s", Err(Error::IntervalZero))]
    #[case("1s", Ok(ValidatedContainerdebugConfig::Enabled { interval: Duration::from_secs(1) }))]
    fn validate_rejects_zero_interval(
        #[case] interval: &str,
        #[case] expected: Result<ValidatedContainerdebugConfig, Error>,
    ) {
        assert_eq!(config(true, interval).validate(), expected);
    }

    /// A role group can disable `containerdebug` without failing on an interval inherited from
    /// its role.
    #[test]
    fn disabled_ignores_interval_and_renders_nothing() {
        let validated = config(false, "0s")
            .validate()
            .expect("the interval is not validated while disabled");

        assert_eq!(validated.background_command(LOG_DIR), None);
    }
}
