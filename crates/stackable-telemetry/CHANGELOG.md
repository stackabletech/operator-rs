# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- Add support for setting the environment variable for each configured tracing subscriber ([#801]).
- Use OpenTelemetry Context in Axum instrumentation layer, adjust log and span level, simplify trace config ([#811]).
  - tracing: Upgrade opentelemetry crates, simplify trace config, fix shutdown conditions, use new way to shutdown LoggerProvider.
  - instrumentation/axum: demote event severity for errors easily caused by clients, replace parent span context if given in http header and link to previous trace contexts.
- Bump rust-toolchain to 1.79.0 ([#822])

[#801]: https://github.com/stackabletech/operator-rs/pull/801
[#811]: https://github.com/stackabletech/operator-rs/pull/811
[#822]: https://github.com/stackabletech/operator-rs/pull/xxx

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
