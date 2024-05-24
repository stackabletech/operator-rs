# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

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
