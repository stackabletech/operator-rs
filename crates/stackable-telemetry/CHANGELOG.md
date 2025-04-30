# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.6.0] - 2025-04-14

### Added

- Add support for JSON console log output ([#1012]).
  - A new CLI argument was added: `--console-log-format`. It can be set to `plain` (default),
    or `json`.

### Changed

- BREAKING: Update and align telemetry related CLI arguments in `TelemetryOptions` ([#1009]).
  - `--console-log-disabled` instead of `--no-console-output`.
  - `--file-log-directory` instead of `--rolling-logs`.
  - `--file-log-rotation-period` instead of `--rolling-logs-period`.
  - `--otel-log-exporter-enabled` instead of `--otlp-logs`.
  - `--otel-trace-exporter-enabled` instead of `--otlp-traces`.
- BREAKING: Update and align telemetry related environment variables ([#1009]).
  - `CONSOLE_LOG_LEVEL` instead of `CONSOLE_LOG`.
  - `FILE_LOG_LEVEL` instead of `FILE_LOG`.
  - `OTEL_LOG_EXPORTER_LEVEL` instead of `OTLP_LOG`.
  - `OTEL_TRACE_EXPORTER_LEVEL` instead of `OTLP_TRACE`.
- BREAKING: Allow configuration of `file_log_max_files` ([#1010]).
  - Adds the `--file-log-max-files` CLI argument (env: `FILE_LOG_MAX_FILES`).
  - `FileLogSettingsBuilder::with_max_log_files` which took a `usize` was renamed to
    `FileLogSettingsBuilder::with_max_files` and now takes an `impl Into<Option<usize>>`
    for improved builder ergonomics.
- Bump `opentelemetry` and related crates to `0.29.x` and `tracing-opentelemetry` to `0.30.0` ([#1021]).

[#1009]: https://github.com/stackabletech/operator-rs/pull/1009
[#1010]: https://github.com/stackabletech/operator-rs/pull/1010
[#1012]: https://github.com/stackabletech/operator-rs/pull/1012
[#1021]: https://github.com/stackabletech/operator-rs/pull/1021

## [0.5.0] - 2025-04-08

### Added

- Add new `Tracing::pre_configured` method ([#1001]).
  - Add `TelemetryOptions` struct and `RollingPeriod` enum
  - Add `clap` feature to enable `TelemetryOptions` being used as CLI arguments

### Changed

- BREAKING: Change `FileLogSettingsBuilder::with_rotation_period` to take `impl Into<Rotation>`
  instead of `Rotation` ([#1001]).

[#1001]: https://github.com/stackabletech/operator-rs/pull/1001

## [0.4.0] - 2025-04-02

### Added

- BREAKING: Allow customization of the rolling file appender [#995].
  - Add required `filename_suffix` field.
  - Add `with_rotation_period` method.
  - Add `with_max_log_files` method.

### Changed

- Bump OpenTelemetry related dependencies ([#977]).
  - `opentelemetry` to 0.28.0
  - `opentelemetry_sdk` to 0.28.0
  - `opentelemetry-appender-tracing` to 0.28.0
  - `opentelemetry-otlp` to 0.28.0
  - `opentelemetry-semantic-conventions` to 0.28.0
  - `tracing-opentelemetry` to 0.29.0

[#977]: https://github.com/stackabletech/operator-rs/pull/977
[#995]: https://github.com/stackabletech/operator-rs/pull/995

## [0.3.0] - 2025-01-30

### Added

- Allow `Option<_>` to be used to enable/disable a subscriber ([#951]).
- Introduce common `Settings` and subscriber specific settings ([#901]).
- Add support for logging to files ([#933]).

### Changed

- BREAKING: Change subscriber settings into an enum to indicate if the subscriber is enabled/disabled ([#951]).
- BREAKING: Rename `TracingBuilder` methods with long names, and prefix with `with_` ([#901]).
- BREAKING: Use the new subscriber settings in the `TracingBuilder` ([#901]).

### Removed

- BREAKING: Remove `Deref` impls for subscriber settings and removed the `enabled` fields and `enabled()` methods ([#951]).

[#901]: https://github.com/stackabletech/operator-rs/pull/901
[#933]: https://github.com/stackabletech/operator-rs/pull/933
[#951]: https://github.com/stackabletech/operator-rs/pull/951

## [0.2.0] - 2024-07-10

### Changed

- BREAKING: Add support for setting the environment variable for each configured tracing subscriber ([#801]).
- Use OpenTelemetry Context in Axum instrumentation layer, adjust log and span level, simplify trace config ([#811]).
  - tracing: Upgrade opentelemetry crates, simplify trace config, fix shutdown conditions, use new way to shutdown LoggerProvider.
  - instrumentation/axum: demote event severity for errors easily caused by clients, replace parent span context if given in http header and link to previous trace contexts.
- Bump rust-toolchain to 1.79.0 ([#822]).

[#801]: https://github.com/stackabletech/operator-rs/pull/801
[#811]: https://github.com/stackabletech/operator-rs/pull/811
[#822]: https://github.com/stackabletech/operator-rs/pull/822

## [0.1.0] - 2024-05-08

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).
- Bump GitHub workflow actions ([#772]).
- Revert `zeroize` version bump ([#772]).

### Fixed

- Prevent infinite events being exported via OTLP, as described in [open-telemetry/opentelemetry-rust#761] ([#796]).

[#772]: https://github.com/stackabletech/operator-rs/pull/772
[#782]: https://github.com/stackabletech/operator-rs/pull/782
[#796]: https://github.com/stackabletech/operator-rs/pull/796
[open-telemetry/opentelemetry-rust#761]: https://github.com/open-telemetry/opentelemetry-rust/issues/761
