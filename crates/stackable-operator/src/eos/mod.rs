use chrono::{DateTime, Utc};
use snafu::{ResultExt, Snafu};
use stackable_shared::time::Duration;
use tracing::{Level, instrument};

/// Available options to configure a [`EndOfSupportChecker`].
///
/// Additionally, this struct can be used as operator CLI arguments. This functionality is only
/// available if the feature `clap` is enabled.
#[cfg_attr(feature = "clap", derive(clap::Args))]
#[derive(Debug, PartialEq, Eq)]
pub struct EndOfSupportOptions {
    /// The end-of-support check mode. Currently, only "offline" is supported.
    #[cfg_attr(feature = "clap", arg(
        long = "eos-check-mode",
        env = "EOS_CHECK_MODE",
        default_value_t = EndOfSupportCheckMode::default(),
        value_enum
    ))]
    pub check_mode: EndOfSupportCheckMode,

    /// The interval in which the end-of-support check should run.
    #[cfg_attr(feature = "clap", arg(
        long = "eos-interval",
        env = "EOS_INTERVAL",
        default_value_t = Self::default_interval()
    ))]
    pub interval: Duration,

    /// The support duration (how long the operator should be considered supported after
    /// it's built-date).
    ///
    /// This field is currently not exposed as a CLI argument or environment variable.
    #[cfg_attr(feature = "clap", arg(skip = Self::default_support_duration()))]
    pub support_duration: Duration,
}

impl EndOfSupportOptions {
    fn default_interval() -> Duration {
        if cfg!(debug_assertions) {
            Duration::from_secs(30)
        } else {
            Duration::from_days_unchecked(1)
        }
    }

    fn default_support_duration() -> Duration {
        Duration::from_days_unchecked(365)
    }
}

#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EndOfSupportCheckMode {
    #[default]
    Offline,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to parse built-time"))]
    ParseBuiltTime { source: chrono::ParseError },
}

pub struct EndOfSupportChecker {
    datetime: DateTime<Utc>,
    interval: Duration,
}

impl EndOfSupportChecker {
    /// Creates and returns a new end-of-support checker.
    ///
    /// - The `built_time` string indicates when a specific operator was built. It is recommended
    ///   to use `built`'s `BUILT_TIME_UTC` constant.
    /// - The `options` allow customizing the checker. It is recommended to use values provided by
    ///   CLI args, see [`EndOfSupportOptions`], [`MaintenanceOptions`](crate::cli::MaintenanceOptions),
    ///   and [`RunArguments`](crate::cli::RunArguments).
    pub fn new(built_time: &str, options: EndOfSupportOptions) -> Result<Self, Error> {
        let EndOfSupportOptions {
            interval,
            support_duration,
            ..
        } = options;

        // Parse the built-time from the RFC2822-encoded string when this is compiled as a release
        // build. If this is a debug/dev build, use the current datetime instead.
        let mut datetime = if cfg!(debug_assertions) {
            Utc::now()
        } else {
            DateTime::parse_from_rfc2822(built_time)
                .context(ParseBuiltTimeSnafu)?
                .to_utc()
        };

        // Add the support duration to the built date. This marks the end-of-support date.
        datetime += *support_duration;

        Ok(Self { datetime, interval })
    }

    /// Run the end-of-support checker.
    ///
    /// It is recommended to run the end-of-support checker via [`futures::try_join!`] or
    /// [`tokio::join`] alongside other futures (eg. for controllers).
    pub async fn run(self) {
        // Construct an interval which can be polled.
        let mut interval = tokio::time::interval(self.interval.into());

        loop {
            // TODO: Add way to stop from the outside
            // The first tick ticks immediately.
            interval.tick().await;
            tracing::info_span!(
                "checking end-of-support state",
                eos.interval = self.interval.to_string(),
            );

            // Continue the loop and wait for the next tick to run the check again.
            if !self.is_eos() {
                continue;
            }

            self.emit_warning();
        }
    }

    /// Emits the end-of-support warning.
    #[instrument(level = Level::DEBUG, skip(self))]
    fn emit_warning(&self) {
        tracing::warn!(
            eos.date = self.datetime.to_rfc3339(),
            "the operator reached end-of-support"
        );
    }

    /// Returns if the operator is considered as end-of-support based on the built-time and the
    /// support duration.
    #[instrument(level = Level::DEBUG, skip(self), fields(eos.now))]
    fn is_eos(&self) -> bool {
        let now = Utc::now();

        tracing::Span::current().record("eos.now", now.to_rfc3339());

        now > self.datetime
    }
}
