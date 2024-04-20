# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Instrument `WebhookServer` with `AxumTraceLayer`, add static healthcheck without instrumentation ([#758]).
- Add shutdown signal hander for the `WebhookServer` ([#767]).

### Changed

- Bump kube to 0.89.0 and update all dependencies ([#762]).

[#758]: https://github.com/stackabletech/operator-rs/pull/758
[#762]: https://github.com/stackabletech/operator-rs/pull/762
[#767]: https://github.com/stackabletech/operator-rs/pull/767

## [0.2.0] - 2024-03-26

### Changed

- Implement `PartialEq` for most _Snafu_ Error enums ([#757]).
- Update Rust to 1.77 ([#759])

[#757]: https://github.com/stackabletech/operator-rs/pull/757
[#759]: https://github.com/stackabletech/operator-rs/pull/759
