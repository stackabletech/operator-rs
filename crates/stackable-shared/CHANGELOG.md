# Changelog

All notable changes to this project will be documented in this file.

## [0.0.2] - 2025-08-21

### Added

- Some modules have been moved into the `stackable-shared` crate, so that they can also be
  used in `stackable-certs` and `stackable-webhook` ([#1074]):
  - The module `stackable_operator::time` has moved to `stackable_operator::shared::time`
  - The module `stackable_operator::commons::secret` has moved to `stackable_operator::shared::secret`

[#1074]: https://github.com/stackabletech/operator-rs/pull/1074

## [0.0.1]

### Added

- Add YAML and CRD helper functions and traits ([#883]).

[#883]: https://github.com/stackabletech/operator-rs/pull/883
