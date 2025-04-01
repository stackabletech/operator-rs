# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Allow customization of the rolling file appender [#995].
  - Add required `filename_suffix` field.
  - Add `with_rotation_period` method.
  - Add `with_max_log_files` method.

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
