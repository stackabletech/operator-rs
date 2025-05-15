# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Add support for Serialization and Deserialization via `serde`. This feature is enabled via the
  `serde` feature flag ([#1034]).

[#1034]: https://github.com/stackabletech/operator-rs/pull/1034

## [0.1.2] - 2024-09-19

### Changed

- Replace `lazy_static` with `std::cell::LazyCell` ([#827], [#835], [#840]).

[#827]: https://github.com/stackabletech/operator-rs/pull/827
[#835]: https://github.com/stackabletech/operator-rs/pull/835
[#840]: https://github.com/stackabletech/operator-rs/pull/840

## [0.1.1] - 2024-07-10

### Changed

- Bump rust-toolchain to 1.79.0 ([#822]).

[#822]: https://github.com/stackabletech/operator-rs/pull/822

## [0.1.0] - 2024-05-08

### Changed

- Bump Rust dependencies and GitHub Actions ([#782]).

[#782]: https://github.com/stackabletech/operator-rs/pull/782
